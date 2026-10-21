use morphforge_app::text_wire;
use morphforge_core::text_panel::TextPanel;
use serde_json::json;
fn main() {
    let panels = vec![
        TextPanel {
            text: "ABC 123 中文\n日本語 Test 456\n第二页字形保持".into(),
            size: [160., 48.],
            ..Default::default()
        },
        TextPanel {
            id: "jp".into(),
            font: "pixel16-jp".into(),
            position: [0., 80., 0.],
            text: "日本語 こんにちは\nWorld!".into(),
            ..Default::default()
        },
    ];
    let mut seq = 1;
    let mut items = vec![];
    for (slot, p) in panels.iter().enumerate() {
        for m in text_wire::encode(p, slot as u8).unwrap() {
            items.push(json!({"data":m.0.to_vec(),"packet":m.packet(seq)}));
            seq = (seq + 1) % 16;
        }
    }
    let mut snap = [0u8; 29];
    let chars = "实时字幕 ABC 日本語".encode_utf16().collect::<Vec<_>>();
    for (i, c) in chars.iter().take(14).enumerate() {
        snap[i * 2..i * 2 + 2].copy_from_slice(&c.to_le_bytes())
    }
    snap[28] = 128 + chars.len().min(14) as u8;
    let mut values = snap
        .iter()
        .enumerate()
        .map(|(i, b)| {
            (
                format!("MF6_D{i:02}"),
                morphforge_core::protocol::OscValue::Int(*b as i32),
            )
        })
        .collect::<Vec<_>>();
    values.push((
        "MF6_Ctl".into(),
        morphforge_core::protocol::OscValue::Int(0),
    ));
    let snapshot = morphforge_core::protocol::bundle(&values);
    let mut deleted = [0u8; 29];
    deleted[0] = 4;
    deleted[2] = 1;
    deleted[28] = 254;
    let delete_packet = text_wire::Message(deleted).packet(seq);
    values.last_mut().unwrap().1 = morphforge_core::protocol::OscValue::Int(64);
    let fresh_snapshot = morphforge_core::protocol::bundle(&values);
    let mut config=text_wire::encode(&panels[0],0).unwrap().pop().unwrap();config.0[24]=chars.len()as u8;config.0[25]=1;
    let restored_config=config.packet((seq+2)%16);config.0[23]=0;config.0[20..23].fill(0);let zero_config=config.packet((seq+1)%16);
    let mut zero_words=[0u8;29];for(i,c)in [65u16,0x4e00,0x0100].iter().enumerate(){zero_words[i*2..i*2+2].copy_from_slice(&c.to_le_bytes())}zero_words[28]=131;
    let zero_snapshot=text_wire::Snapshot{slot:0,epoch:1,data:zero_words}.packet();

    let checks = panels
        .iter()
        .enumerate()
        .map(|(slot, p)| json!({"slot":slot,"characters":text_wire::characters(p).unwrap().0}))
        .collect::<Vec<_>>();
    let file = std::env::args().nth(1).expect("output path");
    std::fs::write(file,serde_json::to_vec_pretty(&json!({"frames":items,"checks":checks,"snapshot":snapshot,"snapshotCharacters":chars,"nextSeq":seq,"deletePacket":delete_packet,"freshSnapshot":fresh_snapshot,"zeroConfig":zero_config,"restoredConfig":restored_config,"zeroSnapshot":zero_snapshot})).unwrap()).unwrap();
}
