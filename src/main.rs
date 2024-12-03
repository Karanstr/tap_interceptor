use uinput::event::keyboard::Key;
use btleplug::api::{Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, WriteType};
use btleplug::platform::{Manager, Peripheral};
use uuid::Uuid;
use tokio::time::{sleep, Duration};
use std::collections::BTreeSet;
use std::error::Error;
use futures_util::stream::StreamExt;


enum Binding {
    PressKey(Key),
    ToggleKey(Key), //This one is a little trickier, maybe we have to save current state of every key in (every profile?)
    SwitchProfile(String),
    Macro, //Figure out what this means
    Empty
}

//Store profiles in a map by name, allowing for plug and play
struct Profile {
    bindings : [Binding; 32],
}


#[tokio::main]
async fn main() {
      
    let default_profile = Profile {
        bindings : [                   //Yeah binary follows the left hand
            Binding::Empty,            //00000 (Default)
            Binding::PressKey(Key::A), //00001
            Binding::PressKey(Key::E), //00010
            Binding::PressKey(Key::N), //00011
            Binding::PressKey(Key::I), //00100
            Binding::PressKey(Key::D), //00101
            Binding::PressKey(Key::T), //00110
            Binding::Empty,            //00111 (Shift)
            Binding::PressKey(Key::O), //01000
            Binding::PressKey(Key::K), //01001
            Binding::PressKey(Key::M), //01010
            Binding::PressKey(Key::F), //01011
            Binding::PressKey(Key::L), //01100
            Binding::PressKey(Key::G), //01101
            Binding::Empty,            //01110 (Backspace)
            Binding::PressKey(Key::R), //01111
            Binding::PressKey(Key::U), //10000
            Binding::PressKey(Key::Y), //10001
            Binding::PressKey(Key::B), //10010
            Binding::PressKey(Key::P), //10011
            Binding::PressKey(Key::Z), //10100
            Binding::PressKey(Key::W), //10101
            Binding::PressKey(Key::Q), //10110
            Binding::PressKey(Key::J), //10111
            Binding::PressKey(Key::S), //11000
            Binding::Empty,            //11001 (Enter)
            Binding::PressKey(Key::X), //11010
            Binding::PressKey(Key::V), //11011
            Binding::Empty,            //11100 (Switch)
            Binding::PressKey(Key::C), //11101
            Binding::PressKey(Key::H), //11110
            Binding::Empty,            //11111 (Space)
        ],
    };

    let tap_service_uuid = Uuid::parse_str("C3FF0001-1D8B-40FD-A56F-C7BD5D0F3370").unwrap();
    
    let tap_data_characteristic = Characteristic {
        uuid : Uuid::parse_str("c3ff0005-1d8b-40fd-a56f-c7bd5d0f3370").unwrap(),
        service_uuid : tap_service_uuid,
        properties : CharPropFlags::NOTIFY,
        descriptors : BTreeSet::new(),
    };

    let mut virtual_keyboard = uinput::open("/dev/uinput")
        .unwrap()
        .name("tap-interceptor")
        .unwrap()
        .event(uinput::event::Keyboard::All)
        .unwrap()
        .create()
        .unwrap();

    let tap = loop {
        if let Some(tap) = get_device_with_service(tap_service_uuid).await {
            break tap
        }
        println!("No compatible devices found. Attempting in 5 seconds");
        tokio::time::sleep(Duration::from_secs(5)).await;
    };
    
    tap.discover_services().await.unwrap();
    let tap_clone = tap.clone();
    let refresh_controller = tokio::spawn(async move {
        let refresh_tap = tap_clone;
        loop {
            match enter_controller(&refresh_tap).await {
                Ok(_) => println!("Refreshed"),
                Err(error) => {dbg!(error);},
            };
            sleep(Duration::from_secs(5)).await;
        }
    });

    tap.subscribe(&tap_data_characteristic).await.unwrap();
    let mut notification_stream = tap.notifications().await.unwrap();

    //Figure out how to detect if the device disconnects
    while let Some(notifications) = notification_stream.next().await {
        match &default_profile.bindings[notifications.value[0] as usize] {
            Binding::Empty => {}
            Binding::PressKey(key) => {
                virtual_keyboard.click(key).unwrap();
                virtual_keyboard.synchronize().unwrap();
            }
            Binding::ToggleKey(key) => {}
            Binding::SwitchProfile(new_profile) => {}
            Binding::Macro => {}
        }
    }
        
    refresh_controller.abort();
    exit_controller(tap).await.unwrap();

}

//Clean these two up
async fn enter_controller(tap:&Peripheral) -> Result<(), Box<dyn Error>> {
    tap.write(&Characteristic {
        uuid : Uuid::parse_str("6E400002-B5A3-F393-E0A9-E50E24DCCA9E").unwrap(),
        service_uuid : Uuid::parse_str("6E400001-B5A3-F393-E0A9-E50E24DCCA9E").unwrap(),
        properties : CharPropFlags::WRITE_WITHOUT_RESPONSE,
        descriptors : BTreeSet::new(),
        },
        &[0x03, 0x0C, 0x00, 0x01], //Magic packet for enter controller
        WriteType::WithoutResponse
    ).await?;
    Ok(())
}

async fn exit_controller(tap:Peripheral) -> Result<(), Box<dyn Error>> {
    tap.write(&Characteristic {
        uuid : Uuid::parse_str("6E400002-B5A3-F393-E0A9-E50E24DCCA9E").unwrap(),
        service_uuid : Uuid::parse_str("6E400001-B5A3-F393-E0A9-E50E24DCCA9E").unwrap(),
        properties : CharPropFlags::WRITE_WITHOUT_RESPONSE,
        descriptors : BTreeSet::new(),
        },
        &[0x03, 0x0C, 0x00, 0x00], //Magic packet for exit controller
        WriteType::WithoutResponse
    ).await?;
    Ok(())
}

async fn get_device_with_service(service_uuid:Uuid) -> Option<Peripheral> {
    let manager = Manager::new().await.unwrap();
    let adapters = manager.adapters().await.unwrap();
    let central = adapters.into_iter().nth(0).unwrap();
    let device_options = central.peripherals().await.unwrap();
    for device in device_options {
        for service in device.properties().await.unwrap().unwrap().services {
            if service == service_uuid {
                if device.is_connected().await.unwrap() {
                    return Some(device)
                }
            }
        }
    }
    None
}

