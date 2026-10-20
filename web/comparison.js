// Explicitly labelled reference composites; never modify generated geometry.
const images = new Map();
export async function referenceImage(id) {
  if (!images.has(id))
    images.set(
      id,
      (async () => {
        const image = new Image();
        image.src = "/api/references/" + encodeURIComponent(id) + "/image";
        await image.decode();
        return image;
      })().catch((e) => {
        images.delete(id);
        throw e;
      }),
    );
  return images.get(id);
}
export function drawReference(ctx, image, c, x, y, width, height) {
  const [sx, sy, sw, sh] = c.crop,
    iw = image.naturalWidth || image.width,
    ih = image.naturalHeight || image.height;
  const fit = Math.min(width / (sw * iw), height / (sh * ih)) * c.zoom,
    w = sw * iw * fit,
    h = sh * ih * fit;
  ctx.save();
  ctx.beginPath();
  ctx.rect(x, y, width, height);
  ctx.clip();
  ctx.drawImage(
    image,
    sx * iw,
    sy * ih,
    sw * iw,
    sh * ih,
    x + (width - w) / 2 + c.offset[0] * width,
    y + (height - h) / 2 + c.offset[1] * height,
    w,
    h,
  );
  ctx.restore();
}
export async function comparisonCanvas(
  render,
  c,
  width = render.width,
  height = render.height,
) {
  const image = await referenceImage(c.imageId),
    canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  ctx.fillStyle = "#101923";
  ctx.fillRect(0, 0, width, height);
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  if (c.mode === "split") {
    const half = Math.floor(width / 2);
    drawReference(ctx, image, c, 0, 0, half, height);
    const fit = Math.min((width - half) / render.width, height / render.height),
      w = render.width * fit,
      h = render.height * fit;
    ctx.drawImage(
      render,
      half + (width - half - w) / 2,
      (height - h) / 2,
      w,
      h,
    );
    ctx.fillStyle = "#eac26e";
    ctx.fillRect(half, 0, 1, height);
  } else {
    ctx.drawImage(render, 0, 0, width, height);
    ctx.globalAlpha = c.opacity;
    drawReference(ctx, image, c, 0, 0, width, height);
    ctx.globalAlpha = 1;
  }
  ctx.fillStyle = "#101923df";
  ctx.fillRect(0, 0, width, 26);
  ctx.fillStyle = "#eac26e";
  ctx.font = "13px sans-serif";
  ctx.fillText(
    c.mode === "split"
      ? "参考图（左） / 作品（右）"
      : "参考图叠加 · 仅用于对照",
    10,
    18,
  );
  return canvas;
}
export async function composeCapture(images, comparison, width, height) {
  if (!comparison) return images;
  return Promise.all(
    images.map(async (image) => {
      const render = new Image();
      render.src = image.data;
      await render.decode();
      const canvas = await comparisonCanvas(
        render,
        comparison,
        width || render.width,
        height || render.height,
      );
      return {
        ...image,
        data: canvas.toDataURL("image/png"),
        rendering: {
          ...image.rendering,
          comparison,
          referenceIsGeometry: false,
        },
      };
    }),
  );
}
