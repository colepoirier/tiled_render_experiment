mod node;
mod pipeline;

pub use node::DownscalingNode;

use bevy::app::prelude::*;
use bevy::asset::{load_internal_asset, HandleUntyped};
use bevy::ecs::prelude::*;
use bevy::log::info;
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::BevyDefault;
use bevy::render::view::ExtractedView;
use bevy::render::{render_phase::DrawFunctions, render_resource::*, RenderApp, RenderStage};

use self::node::DOWNSCALING_PASS;
use bevy::reflect::TypeUuid;

const DOWNSCALING_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 14589267395627146578);
pub struct DownscalingPlugin;

impl Plugin for DownscalingPlugin {
    fn build(&self, app: &mut App) {
        info!("building downscaling plugin!");
        load_internal_asset!(
            app,
            DOWNSCALING_SHADER_HANDLE,
            "downscaling.wgsl",
            Shader::from_wgsl
        );

        let render_app = app.sub_app_mut(RenderApp);

        render_app
            .init_resource::<DrawFunctions<TilePhaseItem>>()
            .add_render_command::<TilePhaseItem, DownscaleTileRenderCommand>()
            .init_resource::<DownscalingPipeline>()
            .init_resource::<SpecializedRenderPipelines<DownscalingPipeline>>()
            .add_system_to_stage(RenderStage::Extract, extract_input_output_images)
            .add_system_to_stage(RenderStage::Prepare, prepare_input_output_textures)
            .add_system_to_stage(RenderStage::Queue, queue_downscaling_bind_groups);

        // connect into the main render graph
        // connect vpull as a node before the main render graph node
        let downscaling_node = DownscalingNode::new(&mut render_app.world);
        let mut graph = render_app.world.resource_mut::<RenderGraph>();
        let draw_2d_graph = graph.get_sub_graph_mut(draw_2d_graph::NAME).unwrap();
        draw_2d_graph.add_node(DOWNSCALING_PASS, downscaling_node);
        draw_2d_graph
            .add_node_edge(DOWNSCALING_PASS, draw_2d_graph::node::MAIN_PASS)
            .unwrap();
        // draw_2d_graph
        //     .add_slot_edge(
        //         draw_2d_graph.input_node().unwrap().id,
        //         draw_2d_graph::input::VIEW_ENTITY,
        //         DOWNSCALE_PASS,
        //         VpullPassNode::IN_VIEW,
        //     )
        //     .unwrap();
    }

    // fn build(&self, app: &mut App) {
    //     app.sub_app_mut(RenderApp)
    //         .init_resource::<DownscalingPipeline>()
    //         .init_resource::<SpecializedRenderPipelines<DownscalingPipeline>>()
    //         .add_system_to_stage(RenderStage::Queue, queue_downscaling_bind_groups);
    // }
}

/// HighResTexture and DownscaledTexture should be created by the extract phase
#[derive(Component, Deref, DerefMut)]
pub struct HighResTexture(Texture);

#[derive(Component, Deref, DerefMut)]
pub struct DownscaledTexture(Texture);

#[derive(Component, Deref, DerefMut)]
pub struct HighResImage(Image);

#[derive(Component, Deref, DerefMut)]
pub struct DownscaledImage(Image);
    
// The commands in this function are from the Render sub app, but the queries access
// entities from the main app.
fn extract_input_output_images(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut input_tile_query: Query<&Handle<Image>, With<HighResTileMarker>>,
    mut output_tile_query: Query<&Handle<Image>, With<DownscaledTileMarker>>,
) {
    for input_tile in input_tile_query.iter() {
        for output_tile in output_tile_query.iter() {
            commands.spawn_bundle(HighResImage(images.get(input_tile)).unwrap().clone(), DownscaledImage(images.get(output_tile).unwrap().clone()));
        }
    }
//     for (entity, input_tile) in input_tile_query.iter_mut() {
//         for input
//         command.spawn_bundle(())
//         if !batched_quads.extracted {
//             let extracted_quads = ExtractedQuads {
//                 data: batched_quads.data.clone(),
//                 prepared: false,
//             };
//             commands
//                 .get_or_spawn(entity)
//                 .insert(extracted_quads.clone());
//             batched_quads.extracted = true;
//             info!("finished extracting quads.");
//         } else {
//             commands.get_or_spawn(entity).insert(ExtractedQuads {
//                 data: Vec::new(),
//                 prepared: true,
//             });
//         }
//     }
// }
}

fn prepare_input_output_textures(mut commands: Commands, render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>, mut input_output_images: Query<(Entity, &HighResImage, &DownscaledImage)>,) {
    for (entity, high_res_image, downscaled_image) in input_output_images.iter() {
        // we really don't want to keep creating textures every time...
        let high_res_texture = render_device.create_texture_with_data(render_queue, *high_res_image.texture_descriptor, *high_res_image.data);
        let downscaled_texture = render_device.create_texture(*downscaled_image.texture_descriptor);
        commands.spawn_bundle((HighResTexture(high_res_texture), DownscaledTexture(downscaled_texture)));
    }
}

fn queue_downscaling_bind_groups(
    mut commands: Commands,
    mut pipeline_cache: ResMut<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<DownscalingPipeline>>,
    downscaling_pipeline: Res<DownscalingPipeline>,
    input_output_texviews: Query<Entity, (With<HighResTexture>, With<DownscaledTexture>)>,
) {
    for entity in input_output_texviews.iter() {
        let key = DownscalingPipelineKey::from_downscaling_mode(DownscalingMode::Filtering);
        let pipeline = pipelines.specialize(&mut pipeline_cache, &downscaling_pipeline, key);

        commands
            .entity(entity)
            .insert(SpecializedDownscalingPipeline { pipeline });
    }
}
