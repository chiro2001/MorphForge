export function captureSampling(width, height, requested = 2, maxSide = 8192) {
  if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width>maxSide||height>maxSide) throw new Error("截图尺寸超出设备渲染能力");
  const factor = Math.max(1, Math.min(3, Number(requested) || 1, Math.floor(maxSide / width), Math.floor(maxSide / height), Math.floor(Math.sqrt(16_777_216 / (width * height)))));
  return { factor, width: width * factor, height: height * factor };
}
export function viewPixelRatio(quality, deviceRatio = 1) {
  return quality === "high" ? 2 : Math.min(1.5, Math.max(1, deviceRatio));
}
