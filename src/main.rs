use magma_renderer::{window,core::*};

mod game;
mod render;
mod util;



fn main() -> eyre::Result<()>{
    // magma_renderer::engine::material::foo();
    // return Ok(());

    let mut window = window::Window::new()?;
    let core = window.core.clone();

    let mut game = game::Game::new(&core, &window.renderpass);
    // window.lock_cursor();
    while window.prepare_and_poll_events()? {
        let mut cmd = CommandBuffer::new(&core);
        cmd.begin()?;

        game.tick(window.delta_time(), &mut cmd, &mut window)?;
        cmd.end()?;
        window.submit_and_present(cmd)?;
    }

    println!("exiting");

    Ok(())
}