use std::sync::Mutex;

use bevy::ecs::prelude::*;
use bevy::ecs::query::QueryState;
use bevy::render::{
    render_graph::{Node, NodeRunError, RenderGraphContext, SlotInfo, SlotType},
    render_resource::{
        BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, LoadOp, Operations,
        PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, SamplerDescriptor,
        TextureViewId,
    },
    renderer::RenderContext,
    view::{ExtractedView, ViewTarget},
};

use super::{DownscalingPipeline, SpecializedDownscalingPipeline};

pub const DOWNSCALING_PASS: &str = "DOWNSCALING_PASS";

pub struct DownscalingNode {
    query: QueryState<(
        &'static InputTextureView,
        &'static OutputTextureView,
        &'static SpecializedDownscalingPipeline,
    )>,
    cached_texture_bind_group: Mutex<Option<(TextureViewId, BindGroup)>>,
}

impl DownscalingNode {
    pub const IN_VIEW: &'static str = "view";

    pub fn new(world: &mut World) -> Self {
        Self {
            query: QueryState::new(world),
            cached_texture_bind_group: Mutex::new(None),
        }
    }
}

impl Node for DownscalingNode {
    fn input(&self) -> Vec<SlotInfo> {
        vec![SlotInfo::new(DownscalingNode::IN_VIEW, SlotType::Entity)]
    }

    fn update(&mut self, world: &mut World) {
        self.query.update_archetypes(world);
    }

    fn run(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let view_entity = graph.get_input_entity(Self::IN_VIEW)?;

        let pipeline_cache = world.get_resource::<PipelineCache>().unwrap();
        let downscaling_pipeline = world.get_resource::<DownscalingPipeline>().unwrap();

        let (orig_texview, downscaled_, xviewspecialized_downscaling_pipeline) =
            match self.query.get_manual(world, view_entity) {
                Ok(query) => query,
                Err(_) => return Ok(()),
            };

        // // target.view is the camera's "output texture",
        // // target.sampled_target will be our downscaled texture view
        // let orig_texview = &view_target.view;
        // let downscaled_texview = &view_target.sampled_target.as_ref().unwrap();

        let mut cached_bind_group = self.cached_texture_bind_group.lock().unwrap();
        let bind_group = match &mut *cached_bind_group {
            Some((id, bind_group)) if orig_texview.id() == *id => bind_group,
            cached_bind_group => {
                let sampler = render_context
                    .render_device
                    .create_sampler(&SamplerDescriptor::default());

                let bind_group =
                    render_context
                        .render_device
                        .create_bind_group(&BindGroupDescriptor {
                            label: None,
                            layout: &downscaling_pipeline.orig_texture_bind_group,
                            entries: &[
                                BindGroupEntry {
                                    binding: 0,
                                    resource: BindingResource::TextureView(orig_texview),
                                },
                                BindGroupEntry {
                                    binding: 1,
                                    resource: BindingResource::Sampler(&sampler),
                                },
                            ],
                        });

                let (_, bind_group) = cached_bind_group.insert((orig_texview.id(), bind_group));
                bind_group
            }
        };

        let pipeline =
            match pipeline_cache.get_render_pipeline(specialized_downscaling_pipeline.pipeline) {
                Some(pipeline) => pipeline,
                None => return Ok(()),
            };

        let pass_descriptor = RenderPassDescriptor {
            label: Some("downscaling_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &downscaled_texview,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Default::default()), // TODO dont_care
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        };

        let mut render_pass = render_context
            .command_encoder
            .begin_render_pass(&pass_descriptor);

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}
