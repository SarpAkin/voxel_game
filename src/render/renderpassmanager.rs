use std::{
    any::Any,
    collections::HashMap,
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc, Mutex,
    },
};

use magma_renderer::core::*;

use crate::game::Game;

pub trait HasRenderPass: Sync + Send + Any + 'static {
    fn renderpass(&self) -> &dyn Renderpass;
    // fn as_mut_any<'a>(&'a mut self) -> &'a mut dyn Any { self as &'a mut dyn Any }
}

pub trait HasRenderPassWithAny: HasRenderPass + Any {
    fn as_mut_any(&mut self) -> &mut dyn Any;
    fn renderpass_(&self) -> &dyn Renderpass;
}

impl<T: HasRenderPass + Any> HasRenderPassWithAny for T {
    fn as_mut_any(&mut self) -> &mut dyn Any { self }
    fn renderpass_(&self) -> &dyn Renderpass { self.renderpass() }
}

struct RenderPassData {
    renderpass: Box<dyn HasRenderPassWithAny>,
    tasks: Mutex<Vec<SubpassTask>>,
}

impl RenderPassData {
    fn get_renderpass(&self) -> &dyn Renderpass { self.renderpass.renderpass() }
}

struct SubpassData {
    renderpass_name: &'static str,
    subpass_index: u32,
    sender: Mutex<SyncSender<CommandBuffer>>,
}

pub struct SubpassRef<'a> {
    subpass: &'a SubpassData,
    manager: &'a RenderPassManager,
}

pub struct RenderPassManager {
    renderpasses: HashMap<&'static str, RenderPassData>,
    subpasses: HashMap<&'static str, SubpassData>,
    compute_cmds: Mutex<Vec<CommandBuffer>>,
    core: Arc<Core>,
}

pub enum SubpassAction {
    Inline(Box<dyn Fn(&mut CommandBuffer) -> () + Send + Sync>),
    Secondry(&'static str),
}

enum SubpassTask {
    Inline(Box<dyn Fn(&mut CommandBuffer) -> () + Send + Sync>),
    Secondry(Receiver<CommandBuffer>),
}

impl RenderPassManager {
    pub fn new(core: &Arc<Core>) -> RenderPassManager {
        Self {
            renderpasses: HashMap::new(),
            subpasses: HashMap::new(),
            compute_cmds: Mutex::new(vec![]),
            core: core.clone(),
        }
    }
    pub fn core(&self) -> &Arc<Core> { &self.core }

    pub fn submit_compute(&self, cmd: CommandBuffer) { self.compute_cmds.lock().unwrap().push(cmd); }

    pub fn register_renderpass(
        &mut self,
        renderpass: Box<dyn HasRenderPassWithAny>,
        name: &'static str,
        subpasses: Vec<SubpassAction>,
    ) {
        let tasks = subpasses
            .into_iter()
            .enumerate()
            .map(|(i, s)| match s {
                SubpassAction::Inline(f) => SubpassTask::Inline(f),
                SubpassAction::Secondry(sname) => {
                    let (send, recv) = sync_channel(64);
                    self.subpasses.insert(
                        sname,
                        SubpassData { renderpass_name: name, subpass_index: i as u32, sender: Mutex::new(send) },
                    );
                    SubpassTask::Secondry(recv)
                }
            })
            .collect();

        self.renderpasses.insert(name, RenderPassData { renderpass: renderpass, tasks: Mutex::new(tasks) });
    }

    pub fn execute_compute_tasks(&mut self, cmd: &mut CommandBuffer) {
        let cmds = std::mem::replace(self.compute_cmds.lock().unwrap().as_mut(), vec![]);
        cmd.exectue_secondries(cmds);
    }

    pub fn get_renderpass<T: HasRenderPass>(&mut self, name: &'static str) -> Option<&mut T> {
        self.renderpasses.get_mut(name).and_then(|rd| rd.renderpass.as_mut_any().downcast_mut())
    }

    pub fn execute_renderpass(&mut self, cmd: &mut CommandBuffer, name: &'static str) {
        let renderpass = self.renderpasses.get(name).expect("couldn't find renderpass");

        let tasks = renderpass.tasks.lock().unwrap();
        for (i, st) in tasks.iter().enumerate() {
            let inline = match st {
                SubpassTask::Inline(_) => true,
                SubpassTask::Secondry(_) => false,
            };
            if i == 0 {
                renderpass.get_renderpass().begin(cmd.inner(), inline);
            } else {
                renderpass.get_renderpass().next(cmd.inner(), inline);
            }
            match st {
                SubpassTask::Inline(f) => f(cmd),
                SubpassTask::Secondry(receivers) => cmd.exectue_secondries(receivers.try_iter().collect()),
            };

            if tasks.len() - 1 > i {
                renderpass.get_renderpass().next(cmd.inner(), false);
            }
        }

        renderpass.get_renderpass().end(cmd.inner());
    }

    pub fn get_subpass(&self, subpass_name: &'static str) -> Option<SubpassRef> {
        self.subpasses.get(subpass_name).and_then(|s| Some(SubpassRef { manager: self, subpass: s }))
    }
}

impl<'a> SubpassRef<'a> {
    pub fn new_cmd(&self) -> eyre::Result<CommandBuffer> {
        let renderpass = self.manager.renderpasses.get(self.subpass.renderpass_name).unwrap();

        let mut cmd = renderpass.get_renderpass().core().new_secondry_cmd();

        cmd.begin_secondry(Some((renderpass.get_renderpass(), self.subpass.subpass_index)))?;

        Ok(cmd)
    }

    pub fn submit_cmd(&self, cmd: CommandBuffer) -> eyre::Result<()> {
        cmd.end()?;
        self.subpass.sender.lock().unwrap().send(cmd)?;
        Ok(())
    }

    pub fn renderpass(&self) -> &dyn Renderpass {
        self.manager.renderpasses.get(self.subpass.renderpass_name).unwrap().get_renderpass()
    }

    pub(crate) fn subpass_index(&self) -> u32 { self.subpass.subpass_index }
}
