mod node;

pub use node::DownscalingNode;

use bevy::app::prelude::*;
use bevy::asset::{load_internal_asset, HandleUntyped};
use bevy::ecs::prelude::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::BevyDefault;
use bevy::render::view::ExtractedView;
use bevy::render::{render_resource::*, RenderApp, RenderStage};

use bevy::reflect::TypeUuid;

const DOWNSCALING_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 14589267395627146578);
pub struct DownscalingPlugin;

impl Plugin for DownscalingPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            DOWNSCALING_SHADER_HANDLE,
            "downscaling.wgsl",
            Shader::from_wgsl
        );

        app.sub_app_mut(RenderApp)
            .init_resource::<DownscalingPipeline>()
            .init_resource::<SpecializedRenderPipelines<DownscalingPipeline>>()
            .add_system_to_stage(RenderStage::Queue, queue_downscaling_bind_groups);
    }
}

pub struct DownscalingPipeline {
    orig_texture_bind_group: BindGroupLayout,
}

impl FromWorld for DownscalingPipeline {
    fn from_world(render_world: &mut World) -> Self {
        let render_device = render_world.get_resource::<RenderDevice>().unwrap();

        let orig_texture_bind_group =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("downscaling_orig_texture_bind_group_layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        DownscalingPipeline {
            orig_texture_bind_group,
        }
    }
}

#[repr(u8)]
pub enum DownscalingMode {
    Filtering = 0,
    Nearest = 1,
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct DownscalingPipelineKey: u32 {
        const NONE                         = 0;
        const DOWNSCALING_MODE_RESERVED_BITS = DownscalingPipelineKey::DOWNSCALING_MODE_MASK_BITS << DownscalingPipelineKey::DOWNSCALING_MODE_SHIFT_BITS;
    }
}

impl DownscalingPipelineKey {
    const DOWNSCALING_MODE_MASK_BITS: u32 = 0b1111; // enough for 16 different modes
    const DOWNSCALING_MODE_SHIFT_BITS: u32 = 32 - 4;

    pub fn from_downscaling_mode(downscaling_mode: DownscalingMode) -> Self {
        let downscaling_mode_bits = ((downscaling_mode as u32) & Self::DOWNSCALING_MODE_MASK_BITS)
            << Self::DOWNSCALING_MODE_SHIFT_BITS;
        DownscalingPipelineKey::from_bits(downscaling_mode_bits).unwrap()
    }

    pub fn downscaling_mode(&self) -> DownscalingMode {
        let downscaling_mode_bits =
            (self.bits >> Self::DOWNSCALING_MODE_SHIFT_BITS) & Self::DOWNSCALING_MODE_MASK_BITS;
        match downscaling_mode_bits {
            0 => DownscalingMode::Filtering,
            1 => DownscalingMode::Nearest,
            other => panic!("invalid downscaling mode bits in DownscalingPipelineKey: {other}"),
        }
    }
}

impl SpecializedRenderPipeline for DownscalingPipeline {
    type Key = DownscalingPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        // if key.contains(DownscalingMode::Filtering) {
        //     // eventually when we have more than one sampling strategy,
        //     // we can use keys to set particular shader defs so that particular code
        //     // is executed in the shader
        //     // for an example, see: bevy_sprite/src/render/mod.rs
        //     // maybe should put a github link to code here?
        // }
        RenderPipelineDescriptor {
            label: Some("downscaling pipeline".into()),
            layout: Some(vec![self.orig_texture_bind_group.clone()]),
            vertex: VertexState {
                shader: DOWNSCALING_SHADER_HANDLE.typed(),
                shader_defs: Vec::new(),
                entry_point: "vs_main".into(),
                buffers: Vec::new(),
            },
            fragment: Some(FragmentState {
                shader: DOWNSCALING_SHADER_HANDLE.typed(),
                shader_defs: vec![],
                entry_point: "fs_main".into(),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::bevy_default(),
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
        }
    }
}

#[derive(Component)]
pub struct DownscalingTarget {
    pub pipeline: CachedRenderPipelineId,
}

fn queue_downscaling_bind_groups(
    mut commands: Commands,
    mut pipeline_cache: ResMut<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<DownscalingPipeline>>,
    downscaling_pipeline: Res<DownscalingPipeline>,
    view_targets: Query<Entity, With<ExtractedView>>,
) {
    for entity in view_targets.iter() {
        let key = DownscalingPipelineKey::from_downscaling_mode(DownscalingMode::Filtering);
        let pipeline = pipelines.specialize(&mut pipeline_cache, &downscaling_pipeline, key);

        commands
            .entity(entity)
            .insert(DownscalingTarget { pipeline });
    }
}
