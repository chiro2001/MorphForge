//! Persistent, editable textured text, distinct from voxelized font geometry.
use crate::{Bounds,GeometryError,Result};
use serde::{Deserialize,Serialize};
use unicode_normalization::UnicodeNormalization;
fn font()->String{"pixel16-zh".into()}
fn extent()->[f64;2]{[128.,32.]}
fn origin()->[f64;3]{[0.,24.,0.]}
fn size()->f64{8.}
fn white()->[u8;3]{[255;3]}
fn left()->String{"left".into()}
fn top()->String{"top".into()}
fn overflow()->String{"clip".into()}
fn gap()->f64{2.}
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(default,rename_all="camelCase",deny_unknown_fields)]
pub struct TextPanel {
 pub id:String,pub name:String,pub text:String,pub group:String,
 #[serde(default="font")]pub font:String,
 #[serde(default="origin")]pub position:[f64;3],pub rotation:[f64;3],
 #[serde(default="extent")]pub size:[f64;2],
 #[serde(default="size")]pub font_size:f64,
 pub letter_spacing:f64,#[serde(default="gap")]pub line_gap:f64,
 #[serde(default="left")]pub align:String,#[serde(default="top")]pub vertical_align:String,
 #[serde(default="overflow")]pub overflow:String,
 #[serde(default="white")]pub color:[u8;3],pub background:[u8;4],
 pub opacity:f64,
}
impl Default for TextPanel {
 fn default()->Self{Self{id:"text".into(),name:"文本框".into(),text:String::new(),group:"text".into(),font:font(),position:origin(),rotation:[0.;3],size:extent(),font_size:size(),letter_spacing:0.,line_gap:gap(),align:left(),vertical_align:top(),overflow:overflow(),color:white(),background:[0;4],opacity:1.}}
}
impl TextPanel {
 pub fn validate(&self)->Result<()> {
  if self.id.is_empty()||self.id.len()>64||!self.id.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_')){return Err(GeometryError("文本框id需要1..64个英文字母/数字/-/_".into()))}
  if self.name.len()>256||self.group.len()>128||self.text.len()>16384||self.text.chars().count()>4096{return Err(GeometryError("文本框内容超过资源限制".into()))}
  if !matches!(self.font.as_str(),"pixel16-zh"|"pixel16-jp")||!matches!(self.align.as_str(),"left"|"center"|"right")||!matches!(self.vertical_align.as_str(),"top"|"middle"|"bottom")||!matches!(self.overflow.as_str(),"clip"|"tail"|"shrink"){return Err(GeometryError("文本框字体或排版选项无效".into()))}
  if self.position.iter().chain(self.rotation.iter()).any(|v|!v.is_finite()||v.abs()>1_000_000.)||self.size.iter().any(|v|!v.is_finite()||*v<=0.||*v>65536.)||!self.font_size.is_finite()||!(0.015625..=1024.).contains(&self.font_size)||!self.line_gap.is_finite()||!(0.0..=1024.).contains(&self.line_gap)||!self.letter_spacing.is_finite()||!(0.0..=1024.).contains(&self.letter_spacing)||!self.opacity.is_finite()||!(0.0..=1.0).contains(&self.opacity){return Err(GeometryError("文本框尺寸/位置/间距参数无效".into()))}
  Ok(())
 }
 pub fn corners(&self)->Vec<[f64;3]> {
  let r=self.rotation.map(|v|v.to_radians()/2.);let [x,y,z]=r.map(f64::sin);let [cx,cy,cz]=r.map(f64::cos);
  let q=[x*cy*cz+cx*y*z,cx*y*cz-x*cy*z,cx*cy*z+x*y*cz,cx*cy*cz-x*y*z];
  [-self.size[0]/2.,self.size[0]/2.].into_iter().flat_map(|x|[0.,self.size[1]].into_iter().map(move|y|[x,y,0.])).map(|p|{let [x,y,z,w]=q;let c=[y*p[2]-z*p[1],z*p[0]-x*p[2],x*p[1]-y*p[0]];let c2=[y*c[2]-z*c[1],z*c[0]-x*c[2],x*c[1]-y*c[0]];std::array::from_fn(|a|self.position[a]+p[a]+2.*w*c[a]+2.*c2[a])}).collect()
 }
 pub fn bounds(&self)->Bounds{let points=self.corners();Bounds{min:std::array::from_fn(|a|points.iter().map(|p|p[a]).fold(f64::INFINITY,f64::min).floor() as i32),max:std::array::from_fn(|a|points.iter().map(|p|p[a]).fold(f64::NEG_INFINITY,f64::max).ceil() as i32)}}
}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Glyph {pub glyph:u16,pub x:f64,pub y:f64,pub width:f64,pub height:f64,pub pixels:u8,pub line:usize}
#[derive(Clone,Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Layout {pub glyphs:Vec<Glyph>,pub line_count:usize,pub visible_lines:usize,pub clipped:bool,pub missing:Vec<char>,pub font_size:f64,pub width:f64,pub height:f64}
pub fn layout(panel:&TextPanel,widths:&[u8])->Result<Layout> {
 panel.validate()?;if widths.len()!=65536{return Err(GeometryError("点阵宽度表长度错误".into()))}
 let mut font_size=panel.font_size;
 for attempt in 0..16 {
  let scale=font_size/16.;let mut lines:Vec<Vec<(u16,u8,f64)>>=vec![vec![]];let mut line_widths=vec![0.];let mut missing=vec![];
  let characters=panel.text.replace("\r\n","\n").replace('\r',"\n").nfc().collect::<Vec<_>>();
  for (character_index,&c) in characters.iter().enumerate(){
   if c=='\n'{lines.push(vec![]);line_widths.push(0.);continue}
   if c.is_control()&&c!='\t'{continue}
   let c=if c=='\t'{' '}else{c};let cp=c as usize;let(glyph,pixels)=if cp<65536&&matches!(widths[cp],8|16){(cp as u16,widths[cp])}else{if !missing.contains(&c){missing.push(c)}(0x25a1,16)};
   let advance=pixels as f64*scale+panel.letter_spacing;let last=lines.len()-1;
   if c.is_ascii_alphanumeric() && (character_index==0 || !characters[character_index-1].is_ascii_alphanumeric()) {
    let word_width=characters[character_index..].iter().take_while(|c|c.is_ascii_alphanumeric()).count() as f64*(8.*scale+panel.letter_spacing)-panel.letter_spacing;
    if !lines[last].is_empty() && word_width<=panel.size[0] && line_widths[last]+word_width>panel.size[0]+1e-9 {lines.push(vec![]);line_widths.push(0.);}
   }
   let last=lines.len()-1;
   if !lines[last].is_empty()&&line_widths[last]+advance-panel.letter_spacing>panel.size[0]+1e-9 {
    let previous=lines[last].last().copied().unwrap();
    let closing="，。！？、；：）》】」』〕〉,.!?;:".contains(c);
    let opening="（《【「『〔〈([{".contains(char::from_u32(previous.0 as u32).unwrap_or(' '));
    let move_previous=(closing||opening) && (previous.1 as f64+pixels as f64)*scale+panel.letter_spacing<=panel.size[0];
    if move_previous{lines[last].pop();line_widths[last]=previous.2;}
    lines.push(vec![]);line_widths.push(0.);
    if move_previous{let next=lines.len()-1;lines[next].push((previous.0,previous.1,0.));line_widths[next]=previous.1 as f64*scale+panel.letter_spacing;}
   }
   let last=lines.len()-1;let x=line_widths[last];lines[last].push((glyph,pixels,x));line_widths[last]+=advance;
  }
  for (line,w)in lines.iter().zip(line_widths.iter_mut()){if !line.is_empty(){*w-=panel.letter_spacing}}
  let line_height=font_size+panel.line_gap;let total_height=lines.len() as f64*line_height-panel.line_gap;
  let max_width=line_widths.iter().copied().fold(0.,f64::max);
  if panel.overflow=="shrink"&&attempt<15&&(total_height>panel.size[1]||max_width>panel.size[0]){
   font_size=(font_size*(panel.size[1]/total_height).min(panel.size[0]/max_width).min(0.95)).max(0.015625);continue
  }
  let capacity=((panel.size[1]+panel.line_gap)/line_height).floor().max(0.)as usize;
  let visible=lines.len().min(capacity);let start=if panel.overflow=="tail"{lines.len().saturating_sub(visible)}else{0};
  let visible_height=visible as f64*line_height-panel.line_gap;
  let top=match panel.vertical_align.as_str(){"bottom"=>visible_height,"middle"=>(panel.size[1]+visible_height)/2.,_=>panel.size[1]};
  let mut glyphs=vec![];
  for (row,line)in lines.iter().enumerate().skip(start).take(visible){let shift=match panel.align.as_str(){"center"=>(panel.size[0]-line_widths[row])/2.,"right"=>panel.size[0]-line_widths[row],_=>0.};for &(glyph,pixels,x)in line{glyphs.push(Glyph{glyph,x:x+shift,y:top-(row-start)as f64*line_height-font_size,width:pixels as f64*scale,height:font_size,pixels,line:row-start});}}
  return Ok(Layout{glyphs,line_count:lines.len(),visible_lines:visible,clipped:lines.len()>visible||max_width>panel.size[0],missing,font_size,width:panel.size[0],height:panel.size[1]})
 }
 unreachable!()
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn mixed_width_wrap_and_tail_are_deterministic(){let mut widths=vec![16;65536];widths['A' as usize]=8;widths['B' as usize]=8;let p=TextPanel{text:"A中B文".into(),font_size:16.,size:[24.,16.],overflow:"tail".into(),..Default::default()};let l=layout(&p,&widths).unwrap();assert_eq!(l.line_count,2);assert_eq!(l.glyphs.iter().map(|g|g.glyph).collect::<Vec<_>>(),vec!['B' as u16,'文' as u16]);assert_eq!(l.glyphs[0].x,0.);assert_eq!(l.glyphs[1].x,8.);assert!(l.clipped);}
 #[test]fn english_words_and_japanese_punctuation_keep_readable_lines(){let mut widths=vec![16;65536];for c in 0..128{widths[c]=8;}let p=TextPanel{text:"a ABC".into(),font_size:16.,size:[32.,64.],..Default::default()};let l=layout(&p,&widths).unwrap();assert_eq!(l.glyphs.iter().find(|g|g.glyph=='A' as u16).unwrap().x,0.);let p=TextPanel{text:"あい。".into(),font_size:16.,size:[32.,64.],..Default::default()};let l=layout(&p,&widths).unwrap();assert_eq!(l.glyphs[1].line,1);assert_eq!(l.glyphs[2].line,1);}
 #[test]fn invalid_dimensions_do_not_enter_persistent_scene(){let p=TextPanel{font_size:f64::NAN,..Default::default()};assert!(p.validate().is_err());}
}
