mod rendering;
use bevy::{
    a11y::AccessibilityPlugin,
    ecs::system::{NonSendMarker, SystemName},
    input::InputPlugin,
    prelude::*,
    window::PrimaryWindow,
    winit::{WINIT_WINDOWS, WinitPlugin},
};

#[derive(Component)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Message)]
struct TestMessage(i32);

use raw_window_handle::HasWindowHandle;


use rendering::rendering_driver::VkRenderingPlugin;

struct EguiPlugin;
#[derive(Resource)]
struct EguiResource {
    ctx: egui::Context,
}
impl Plugin for EguiPlugin {
    fn build(&self, app: &mut App) {
        println!("{:?}", app.sub_apps().main.update_schedule.unwrap());
        let ctx = egui::Context::default();
        app//.add_systems(Update, egui_system)
            .insert_resource(EguiResource { ctx });
    }
}


fn main() {
    App::new()
         .add_plugins(MinimalPlugins)
         .add_plugins(<AssetPlugin>::default())
         .add_plugins(<ImagePlugin>::default())
         .add_plugins(<InputPlugin>::default())
         .add_plugins(<WindowPlugin>::default())
         .add_plugins(<WinitPlugin>::default())
         .add_plugins(<AccessibilityPlugin>::default())
         .add_plugins(EguiPlugin)
         .add_plugins(VkRenderingPlugin)
         .add_message::<TestMessage>()
         .run();
}

fn egui_system(
    world: &World,
    testc: SystemName,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    _non_send_marker: NonSendMarker,
    egui_res: Res<EguiResource>,
    mut cmd: Commands
) {
    println!("{}, {}", testc, world.entity_count());
    WINIT_WINDOWS.with_borrow(|winit_res| {
        if let Some(res) = winit_res.get_window(primary_window.single().unwrap()) {
            let res = res.window_handle().unwrap().as_raw();
        }
    });
    let bevy_window = windows.single().unwrap();
    let mut raw_input = egui::RawInput::default();
    let size = &bevy_window.resolution;
    raw_input.screen_rect = Some(egui::Rect::from_min_size(
        Default::default(),
        egui::Vec2 {
            x: size.width(),
            y: size.height(),
        },
    ));
    let full_output = egui_res.ctx.run(raw_input, |ctx| {
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.label("Hello world!");
            if ui.button("Click me").clicked() {
                // take some action here
            }
        });
    });
    // full_output.platform_output should be handled
    let clipped_prims = egui_res
        .ctx
        .tessellate(full_output.shapes, full_output.pixels_per_point);
    // paint code
}