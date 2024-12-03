use uinput::event::keyboard::Key;
use btleplug::api::{Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, WriteType};
use btleplug::platform::{Manager, Peripheral};
use uuid::Uuid;
use tokio::time::{sleep, Duration};
use std::collections::BTreeSet;
use std::error::Error;
use futures_util::stream::StreamExt;

#[tokio::main]

async fn main() {
    let tap_service_uuid = Uuid::parse_str("C3FF0001-1D8B-40FD-A56F-C7BD5D0F3370").unwrap();
    
    let tap_data_characteristic = Characteristic {
        uuid : Uuid::parse_str("c3ff0005-1d8b-40fd-a56f-c7bd5d0f3370").unwrap(),
        service_uuid : tap_service_uuid,
        properties : CharPropFlags::NOTIFY,
        descriptors : BTreeSet::new(),
    };

    //Virtual keyboard
    /*
    let mut input = uinput::open("/dev/uinput")
        .unwrap()
        .name("tap-interceptor")
        .unwrap()
        .event(uinput::event::Keyboard::All)
        .unwrap()
        .create()
        .unwrap();
    */


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
        dbg!(notifications);
        
    }
        
    refresh_controller.abort();
    exit_controller(tap);

}

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

fn exit_controller(tap:Peripheral) {
    let _ = tap.write(&Characteristic {
        uuid : Uuid::parse_str("6E400002-B5A3-F393-E0A9-E50E24DCCA9E").unwrap(),
        service_uuid : Uuid::parse_str("6E400001-B5A3-F393-E0A9-E50E24DCCA9E").unwrap(),
        properties : CharPropFlags::WRITE_WITHOUT_RESPONSE,
        descriptors : BTreeSet::new(),
        },
        &[0x03, 0x0C, 0x00, 0x00], //Magic packet for exit controller
        WriteType::WithoutResponse
    );
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

