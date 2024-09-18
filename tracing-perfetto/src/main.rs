use bytes::Bytes;
use bytes::BytesMut;
use prost::Message;
use std::env;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::process::Command;
use std::u64;

mod idl {
    include!("perfetto.proto.rs");
}

// bin <debug / release>
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("invalid args!")
    }

    let mode: &str = &args[1];
    println!("args: {:?}", &args);
    let mut file = File::open("/home/ec2-user/tokio/test.pftrace").unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    let bytes = Bytes::from(buf);
    let mut trace = idl::Trace::decode(bytes).unwrap();

    // todo: change the way to get an offset
    let mut offset = None;
    for packet in trace.packet.iter_mut() {
        let Some(data) = &packet.interned_data else {
            continue;
        };
        let Some(interned_string) = data.debug_annotation_string_values.get(0) else {
            continue;
        };
        offset = interned_string
            .str
            .as_ref()
            .map(|v| v.into_iter().fold(0_u64, |acc, x| acc * 10 + (*x as u64)));
    }

    for packet in trace.packet.iter_mut() {
        let Some(idl::trace_packet::Data::TrackEvent(ref mut e)) = &mut packet.data else {
            continue;
        };
        // track_event::Type::SliceBegin
        if e.r#type != Some(1) {
            continue;
        }
        let Some(f) = e.debug_annotations.iter_mut().find(|a| {
            let cond1 = match &a.value {
                Some(v) => match v {
                    idl::debug_annotation::Value::StringValue(v) => v != &"".to_string(),
                    _ => false,
                },
                _ => false,
            };
            let cond2 = a.name_field.as_ref()
                == Some(&idl::debug_annotation::NameField::Name(
                    "stacktrace".to_string(),
                ));

            cond1 && cond2
        }) else {
            continue;
        };

        let Some(idl::debug_annotation::Value::StringValue(ref mut s)) = &mut f.value else {
            panic!("eeeeeee")
        };

        let addresses: Vec<String> = s
            .split(",")
            .filter_map(|s| {
                let Ok(addr) = s.parse::<u64>() else {
                    return None;
                };

                let addr = addr - offset.unwrap();
                Some(format!("{:#x}", addr))
            })
            .collect();

        let target_bin = format!("./target/{mode}/examples/worker-tracing");
        let output = Command::new("addr2line")
            .arg("-C")
            .arg("-f")
            .arg("-e")
            .arg(target_bin)
            .args(&addresses)
            .output()
            .unwrap();

        let output_str = String::from_utf8_lossy(&output.stdout);
        println!("result: {}\n\n\n", output_str);
        *s = output_str.to_string();
    }

    let mut buf = BytesMut::new();
    trace.encode(&mut buf).unwrap();

    let mut file = File::create("/home/ec2-user/tokio/test_symbolize.pftrace").unwrap();

    file.write_all(&buf).unwrap();
}
