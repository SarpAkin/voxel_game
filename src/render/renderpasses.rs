use ash::vk;
use magma_renderer::core::*;
use std::{ops::DerefMut, sync::Arc};

use crate::{
    game::{Game, RenderGlobals},
    include_glsl,
};

use super::renderpassmanager::*;

const CLEAR_ZERO: vk::ClearValue = vk::ClearValue { color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 0.0] } };

pub struct DeferedPass {
    pub renderpass: MultiPassRenderPass,
    pub depth: AttachmentIndex,
    pub normal: AttachmentIndex,
    pub albedo_spec: AttachmentIndex,
    dset_layout: vk::DescriptorSetLayout,
    pipeline: Arc<Pipeline>,
    sampler: Handle<vk::Sampler>,
}

impl HasRenderPass for DeferedPass {
    fn renderpass(&self) -> &dyn Renderpass { &self.renderpass }
}

impl DeferedPass {
    pub fn new(core: &Arc<Core>, rp: &dyn Renderpass) -> DeferedPass {
        let (w, h) = rp.extends();

        let mut gpassbulder = RenderPassBuilder::new();
        let albedo_spec = gpassbulder.add_attachment(vk::Format::R8G8B8A8_UNORM, Some(CLEAR_ZERO), true);
        let depth = gpassbulder.add_attachment(
            vk::Format::D16_UNORM,
            Some(vk::ClearValue { depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 } }),
            false,
        );
        let normal = gpassbulder.add_attachment(vk::Format::R16G16B16A16_SNORM, Some(CLEAR_ZERO), false);
        gpassbulder.add_subpass(&[albedo_spec, normal], Some(depth), &[]);
        let renderpass = gpassbulder.build(core, w, h).unwrap();

        let (pipeline, dset_layout) = Self::create_pipeline(core, rp).unwrap();
        let sampler = core.create_sampler(vk::Filter::NEAREST, None);

        Self { renderpass, albedo_spec, normal, depth, pipeline, dset_layout, sampler }
    }

    fn create_pipeline(core: &Arc<Core>, rp: &dyn Renderpass) -> eyre::Result<(Arc<Pipeline>, vk::DescriptorSetLayout)> {
        let dset_layout = DescriptorSetLayoutBuilder::new().add_sampler(vk::ShaderStageFlags::FRAGMENT, 1).build(core)?;

        let layout = PipelineLayoutBuilder::new().add_set(dset_layout).build(core)?;

        let pipeline = GPipelineBuilder::new()
            .set_depth_testing(false)
            .set_rasterization(vk::PolygonMode::FILL, vk::CullModeFlags::NONE)
            .set_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .set_pipeline_layout(layout)
            .add_shader_stage(
                vk::ShaderStageFlags::VERTEX,
                &ShaderModule::new(core, include_glsl!("res/screen_quad.vert"))?.module(),
            )
            .add_shader_stage(
                vk::ShaderStageFlags::FRAGMENT,
                &ShaderModule::new(core, include_glsl!("res/final.frag"))?.module(),
            )
            .build(core, rp, 0)?;

        Ok((pipeline, dset_layout))
    }

    pub fn register(self, man: &mut RenderPassManager, game: &mut Game) {
        man.register_renderpass(
            Box::new(self),
            "deferred_render",
            vec![
                SubpassAction::Secondry("gpass"), //
            ],
        );
    }

    fn render_to_swapchain(&self,cmd: &mut CommandBuffer,game:&Game)-> eyre::Result<()>{
        let image = self.renderpass.get_attachment(self.albedo_spec);
        let dset = DescriptorSetBuilder::new().add_sampled_image(image, *self.sampler).build(
            self.dset_layout,
            game.world.fetch::<RenderGlobals>().frame_data().descriptor_pool.lock().unwrap().deref_mut(),
        )?;
    
        cmd.bind_pipeline(&self.pipeline);
        cmd.bind_descriptor_set(0, dset);
        unsafe {
            cmd.draw(3, 1, 0, 0);
        }

        Ok(())
    }
}

pub fn init(game: &mut Game, rp: &dyn Renderpass) {
    let mut man = RenderPassManager::new(&game.core);

    DeferedPass::new(&game.core, rp).register(&mut man, game);

    // let gpass = crate::game::DeferedPass::new(&game.core, rp.extends());
    // gpass.register(&man,game);

    game.world.insert(man);
}

pub fn prepare_render(game: &mut Game,rp:&dyn Renderpass) -> eyre::Result<()>{
    let mut man = game.world.fetch_mut::<RenderPassManager>();
    let deferred_renderer = man.get_renderpass::<DeferedPass>("deferred_render").unwrap();
    if deferred_renderer.renderpass.extends() != rp.extends() {
        let (width,height) = rp.extends();
        deferred_renderer.renderpass.resize(width, height)?;
    }

    Ok(())
}

pub fn render(game: &mut Game, cmd: &mut CommandBuffer, rp: &dyn Renderpass) -> eyre::Result<()> {
    let mut man = game.world.fetch_mut::<RenderPassManager>();

    man.execute_compute_tasks(cmd);

    man.execute_renderpass(cmd, "deferred_render");

    rp.begin(cmd.inner(), true);
    let defered_renderer = man.get_renderpass::<DeferedPass>("deferred_render").unwrap();
    defered_renderer.render_to_swapchain(cmd, game)?;

    rp.end(cmd.inner());

    Ok(())
}

