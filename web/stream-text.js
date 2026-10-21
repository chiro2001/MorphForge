import {readableText} from "/readable-text.js";
// A compact transcript keeps its own scroll position, independent of the feed.
export class StreamText {
  constructor(container, { label = "展开正文" } = {}) {
    this.container=container;this.expanded=false;this.following=true;this.text="";this.frame=0;
    this.button=document.createElement("button");this.button.className="stream-expand";
    this.button.textContent=label;this.button.setAttribute("aria-expanded","false");
    this.latest=document.createElement("button");this.latest.textContent="回到最新";this.latest.className="stream-latest";this.latest.hidden=true;
    this.content=document.createElement("div");this.content.className="stream-content";this.content.tabIndex=0;
    container.classList.add("stream-message");container.append(this.button,this.latest,this.content);
    this.button.onclick=()=>{this.expanded=!this.expanded;container.classList.toggle("expanded",this.expanded);this.button.setAttribute("aria-expanded",String(this.expanded));this.button.textContent=this.expanded?"收为单行":label;this.render();};
    this.latest.onclick=()=>{this.following=true;this.latest.hidden=true;this.schedule();};
    const detach=()=>{this.following=false;this.latest.hidden=false;cancelAnimationFrame(this.frame);this.frame=0;};
    this.content.addEventListener("wheel",detach,{passive:true});
    this.content.addEventListener("pointerdown",detach);
    this.content.addEventListener("keydown",e=>{if(["Home","ArrowLeft","ArrowUp","PageUp"].includes(e.key))detach();if(e.key==="End"){this.following=true;this.schedule();}});
    this.content.addEventListener("scroll",()=>{if(!this.following&&!this.frame&&this.atEnd()){this.following=true;this.latest.hidden=true;}});
  }
  atEnd(){const c=this.content;return this.expanded?c.scrollHeight-c.clientHeight-c.scrollTop<3:c.scrollWidth-c.clientWidth-c.scrollLeft<3;}
  setText(text){if(this.text===text)return;this.text=text||"";this.render();}
  render(){if(this.expanded)readableText(this.content,this.text);else this.content.textContent=this.text.replace(/\s*\n\s*/g,"  ");this.schedule();}
  schedule(){if(!this.following||this.frame||!this.container.isConnected)return;const tick=()=>{this.frame=0;if(!this.following||!this.container.isConnected)return;const c=this.content,key=this.expanded?"scrollTop":"scrollLeft",target=this.expanded?c.scrollHeight-c.clientHeight:c.scrollWidth-c.clientWidth;const gap=target-c[key];c[key]+=Math.abs(gap)<1?gap:gap*.3;if(Math.abs(gap)>1)this.frame=requestAnimationFrame(tick);};this.frame=requestAnimationFrame(tick);}
}
