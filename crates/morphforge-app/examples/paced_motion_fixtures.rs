//! Capture the production paced writer, then replay its exact packets in the owned Unity fixture.
use morphforge_app::{avatar_motion,creative::{Clip,Track,Keyframe},server::App,target::AvatarCatalog};
use morphforge_core::{Box3,Scene};
use serde_json::json;
use std::{net::UdpSocket,time::{Duration,Instant}};
#[tokio::main]
async fn main(){
 let dir=tempfile::tempdir().unwrap();let app=App::open(dir.path().into(),None).unwrap();let receiver=UdpSocket::bind("127.0.0.1:0").unwrap();receiver.set_nonblocking(true).unwrap();
 let mut catalog=AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json")).unwrap();catalog.capabilities.push(avatar_motion::CAPABILITY.into());
 {let mut s=app.transport.inner.lock().unwrap();s.avatar_catalog=Some(catalog);s.config.port=receiver.local_addr().unwrap().port();s.config.interval=0.25;}
 let mut scene=Scene::default();scene.revision=1;let mut b=Box3::new([-12,40,-4],[24,4,8],500);b.group="wind".into();scene.boxes.push(b);
 let key=|time,angle|Keyframe{time,rotation:[0.,angle,0.],translation:[0.;3],scale:[1.;3]};
 let clip=Clip{name:"Actual paced native writer".into(),duration:4.,looping:true,tracks:vec![Track{group:"wind".into(),pivot:[0.,42.,0.],interpolation:"linear".into(),keys:vec![key(0.,0.),key(2.,180.),key(4.,360.)]}]};
 let mut materials=morphforge_core::materials::builtins();let mut red=materials[0].clone();red.id=500;red.uid="paced-fixture-red".into();red.color=[0.86,0.31,0.16];materials.push(red);
 app.transport.publish_motion(&scene,&materials,&clip,"paced",1.,0.,true).unwrap();app.transport.inner.lock().unwrap().config.enabled=true;
 let start=Instant::now();let mut rows=vec![];let mut bytes=[0;8192];
 loop {while let Ok(n)=receiver.recv(&mut bytes){rows.push(json!({"time":start.elapsed().as_secs_f64(),"packet":bytes[..n].to_vec()}));}
 if app.transport.state()["sentRevision"]==1 {break;}assert!(start.elapsed()<Duration::from_secs(30));tokio::time::sleep(Duration::from_millis(5)).await;}
 app.transport.inner.lock().unwrap().stop=true;
 std::fs::write(std::env::args().nth(1).expect("output"),serde_json::to_vec_pretty(&json!({"success":true,"source":"App transport production paced writer, 250ms interval","packets":rows,"state":app.transport.state()})).unwrap()).unwrap();
}
