//! Test-only Wayland virtual keyboard and pointer. Use exclusively with tests/headless-ui.sh.
use std::{
    fs::File,
    io::{BufRead, Write},
    os::fd::AsFd,
    time::Duration,
};
use wayland_client::{
    Connection, Dispatch, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry, wl_seat},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};
struct State;
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ZwpVirtualKeyboardManagerV1);
delegate_noop!(State: ZwpVirtualKeyboardV1);
delegate_noop!(State: ZwlrVirtualPointerManagerV1);
delegate_noop!(State: ZwlrVirtualPointerV1);
fn code(key: &str) -> u32 {
    match key {
        "Down" => 108,
        "Up" => 103,
        "Return" => 28,
        "Escape" => 1,
        "BackSpace" => 14,
        "space" => 57,
        _ => {
            let letters = [
                ('q', 16),
                ('w', 17),
                ('e', 18),
                ('r', 19),
                ('t', 20),
                ('y', 21),
                ('u', 22),
                ('i', 23),
                ('o', 24),
                ('p', 25),
                ('a', 30),
                ('s', 31),
                ('d', 32),
                ('f', 33),
                ('g', 34),
                ('h', 35),
                ('j', 36),
                ('k', 37),
                ('l', 38),
                ('z', 44),
                ('x', 45),
                ('c', 46),
                ('v', 47),
                ('b', 48),
                ('n', 49),
                ('m', 50),
            ];
            letters
                .into_iter()
                .find(|(c, _)| key == c.to_string())
                .expect("unsupported test key")
                .1
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("BLASTOFF_TEST_DIR")?;
    let socket = std::env::var("SWAYSOCK")?;
    if !socket.starts_with(&format!("{dir}/")) {
        return Err("virtual keyboard requires the isolated test compositor".into());
    }
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
    let qh = queue.handle();
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=1, ())?;
    let manager: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let pointer_manager: ZwlrVirtualPointerManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let pointer = pointer_manager.create_virtual_pointer(Some(&seat), &qh, ());
    pointer.motion_absolute(0, 0, 0, 1280, 960);
    pointer.frame();
    let keyboard = manager.create_virtual_keyboard(&seat, &qh, ());
    let keymap = b"xkb_keymap { xkb_keycodes { include \"evdev+aliases(qwerty)\" }; xkb_types { include \"complete\" }; xkb_compatibility { include \"complete\" }; xkb_symbols { include \"pc+us+inet(evdev)\" }; };\0";
    let mut file = File::create(format!("{dir}/keymap"))?;
    file.write_all(keymap)?;
    let file = File::open(format!("{dir}/keymap"))?;
    keyboard.keymap(1, file.as_fd(), keymap.len() as u32);
    queue.roundtrip(&mut State)?;
    // Give clients time to bind the newly advertised keyboard capability.
    std::thread::sleep(Duration::from_millis(300));
    keyboard.modifiers(0, 0, 0, 0);
    queue.roundtrip(&mut State)?;
    println!("READY");
    std::io::stdout().flush()?;
    for line in std::io::stdin().lock().lines() {
        for arg in line?.split_whitespace() {
            if let Some(coords) = arg.strip_prefix("pointer:") {
                let (x, y) = coords.split_once(',').ok_or("pointer requires x,y")?;
                pointer.motion_absolute(0, x.parse()?, y.parse()?, 1280, 960);
                pointer.frame();
                queue.roundtrip(&mut State)?;
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            let keys: Vec<String> = if let Some(value) = arg.strip_prefix("text:") {
                value.chars().map(|c| c.to_string()).collect()
            } else {
                vec![arg.to_owned()]
            };
            for key in keys {
                keyboard.key(0, code(&key), 1);
                queue.roundtrip(&mut State)?;
                std::thread::sleep(Duration::from_millis(60));
                keyboard.key(0, code(&key), 0);
                queue.roundtrip(&mut State)?;
                std::thread::sleep(Duration::from_millis(60));
            }
        }
        println!("DONE");
        std::io::stdout().flush()?;
    }
    pointer.destroy();
    keyboard.destroy();
    conn.flush()?;
    Ok(())
}
