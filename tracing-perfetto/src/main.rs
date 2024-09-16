use bytes::Bytes;
use bytes::BytesMut;
use prost::Message;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::process::Command;
use std::u64;

mod idl {
    include!("perfetto.proto.rs");
}

fn main() {
    let mut file = File::open("/home/ec2-user/tokio/test.pftrace").unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    let bytes = Bytes::from(buf);
    let mut trace = idl::Trace::decode(bytes).unwrap();

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

        // 94653671911424
        let addresses: Vec<String> = s
            .split(",")
            .filter_map(|s| {
                let Ok(addr) = s.parse::<u64>() else {
                    return None;
                };

                let addr = addr - 94576022376448;
                Some(format!("{:#x}", addr))
            })
            .collect();

        let output = Command::new("addr2line")
            .arg("-C")
            .arg("-f")
            .arg("-e")
            .arg("./target/debug/examples/worker-tracing")
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
