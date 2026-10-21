// Mono 16kHz frames; output is silent so capture never feeds the loudspeakers.
class MorphForgeCapture extends AudioWorkletProcessor {
 constructor(){super();this.index=0;this.next=0;this.previous=0;this.pending=[];this.ratio=sampleRate/16000;}
 process(inputs){const channels=inputs[0];if(!channels?.length)return true;for(let i=0;i<channels[0].length;i++){let value=0;for(const ch of channels)value+=ch[i]||0;value/=channels.length;while(this.next<=this.index){const fraction=Math.max(0,Math.min(1,this.next-(this.index-1)));this.pending.push(this.previous+(value-this.previous)*fraction);this.next+=this.ratio;if(this.pending.length===1600){const frame=new Float32Array(this.pending);this.pending=[];this.port.postMessage(frame,[frame.buffer]);}}this.previous=value;this.index++;}return true;}
}
registerProcessor('morphforge-capture',MorphForgeCapture);
