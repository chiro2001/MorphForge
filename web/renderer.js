import { fitBounds, resolveCamera } from "/camera-spec.js";
import * as THREE from "three";
import { OrbitControls } from "/vendor/OrbitControls.js";

// Canonical art/VRC coordinates: X right in object space, Y up, +Z forward.
// Three uses the opposite handedness: apply this at every render/pick boundary.
export const toThree = (p) => new THREE.Vector3(p[0], p[1], -p[2]);
export const fromThree = (v) => [v.x, v.y, -v.z];
export const MAX_RESOURCE_FRAMES = 1024;
export function resourceAtlasLayout(
  width,
  height,
  declaredFrames,
  maxTextureSize = 16384,
) {
  const sourceFrames = Math.max(1, Math.floor(height / Math.max(1, width)));
  const declared = Math.max(0, Math.floor(declaredFrames || 0));
  const frames = Math.max(
    1,
    Math.min(MAX_RESOURCE_FRAMES, declared || sourceFrames),
  );
  const resolution = Math.max(
    1,
    Math.min(
      128,
      Math.floor(width) || 1,
      Math.floor(maxTextureSize / 6),
      Math.floor(maxTextureSize / frames),
    ),
  );
  return { frames, resolution };
}
export function resourceFrame(frames, frameTime, time) {
  const count = Math.max(1, Math.floor(frames || 1));
  const duration = Math.max(0.001, Number(frameTime) || 0.1);
  return (
    ((Math.floor(Math.max(0, time) / duration + 1e-9) % count) + count) % count
  );
}
export class VoxelRenderer {
  constructor(container, { interactive = true } = {}) {
    this.container = container;
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color("#101923");
    this.camera = new THREE.PerspectiveCamera(42, 1, 0.005, 5000);
    this.camera.position.copy(toThree([5, 4.3, 6]));
    this.renderer = new THREE.WebGLRenderer({
      antialias: true,
      preserveDrawingBuffer: true,
    });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 1.5));
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.15;
    container.append(this.renderer.domElement);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enabled = interactive;
    this.controls.enableDamping = true;
    this.controls.mouseButtons = {
      LEFT: null,
      MIDDLE: THREE.MOUSE.PAN,
      RIGHT: THREE.MOUSE.ROTATE,
    };
    this.controls.target.copy(toThree([0, 0.8, 0]));
    this.content = new THREE.Group();
    this.scene.add(this.content);
    this.meshes = [];
    this.materials = [];
    this.textures = [];
    this.boxes = [];
    this.groupsHidden = new Set();
    this.cellSize = 0.25;
    this.scale = 1;
    this.mode = "art";
    this.renderRevision = -1;
    this.selection = new Set();
    this.uniforms = [];
    this.scene.add(new THREE.HemisphereLight(0xd6ebff, 0x3b3c36, 1.25));
    const sun = new THREE.DirectionalLight(0xffe4be, 2.4);
    sun.position.set(4, 8, -5);
    this.scene.add(sun);
    const fill = new THREE.DirectionalLight(0x74b6ff, 0.8);
    fill.position.set(-5, 2, 4);
    this.scene.add(fill);
    this.makeEnvironment();
    this.helpers = new THREE.Group();
    this.scene.add(this.helpers);
    this.grid = new THREE.GridHelper(4, 16, 0x596c7a, 0x2c3e4b);
    this.helpers.add(this.grid);
    this.axes = new THREE.AxesHelper(0.4);
    this.axes.scale.z = -1;
    this.helpers.add(this.axes);
    this.marker = new THREE.Mesh(
      new THREE.SphereGeometry(0.045, 20, 12),
      new THREE.MeshStandardMaterial({
        color: "#f6be4c",
        emissive: "#b7750b",
        emissiveIntensity: 1.4,
        transparent: true,
        opacity: 0.65,
      }),
    );
    this.helpers.add(this.marker);
    this.ghost = new THREE.Mesh(
      new THREE.BoxGeometry(1, 1, 1),
      new THREE.MeshBasicMaterial({
        color: 0xecc468,
        transparent: true,
        opacity: 0.25,
        depthWrite: false,
      }),
    );
    this.ghost.visible = false;
    this.helpers.add(this.ghost);
    this.selectedHelper = new THREE.Box3Helper(new THREE.Box3(), 0xf3ca72);
    this.selectedHelper.visible = false;
    this.helpers.add(this.selectedHelper);
    this.textPreviewGroup = new THREE.Group();
    this.textPreviewGroup.name = "TextPreview";
    this.scene.add(this.textPreviewGroup);
    this.textPreviewMesh = null;
    this.textPreviewBoxes = [];
    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(container);
    this.resize();
    this.renderer.setAnimationLoop((time) => {
      this.controls.update();
      for (const uniforms of this.uniforms) uniforms.mfTime.value = time / 1000;
      this.renderer.render(this.scene, this.camera);
    });
  }
  makeEnvironment() {
    const images = [];
    for (let i = 0; i < 6; i++) {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 128;
      const c = canvas.getContext("2d"),
        g = c.createLinearGradient(0, 0, 0, 128);
      g.addColorStop(0, "#a4c9e5");
      g.addColorStop(0.5, "#d4e0e4");
      g.addColorStop(0.51, "#45545d");
      g.addColorStop(1, "#1d2732");
      c.fillStyle = g;
      c.fillRect(0, 0, 128, 128);
      c.fillStyle = i % 2 ? "#dbefff" : "#fff0d7";
      c.fillRect(15, 15, 30, 65);
      images.push(canvas);
    }
    const cube = new THREE.CubeTexture(images);
    cube.needsUpdate = true;
    const generator = new THREE.PMREMGenerator(this.renderer);
    this.environment = generator.fromCubemap(cube);
    this.scene.environment = this.environment.texture;
    cube.dispose();
    generator.dispose();
  }
  resize() {
    const r = this.container.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) return;
    this.renderer.setSize(r.width, r.height);
    this.camera.aspect = r.width / r.height;
    this.camera.updateProjectionMatrix();
  }
  async atlas(style) {
    if (!style.textures?.length) return null;
    const images = await Promise.all(
      style.textures.slice(0, 6).map(
        (url) =>
          new Promise((resolve, reject) => {
            const im = new Image();
            im.onload = () => resolve(im);
            im.onerror = () => reject(new Error("无法加载材质 " + url));
            im.src = url;
          }),
      ),
    );
    while (images.length < 6) images.push(images[0]);
    const sourceFrames = Math.max(
      ...images.map((im) => Math.max(1, Math.floor(im.height / im.width))),
    );
    const declared = Math.max(
      0,
      Math.min(MAX_RESOURCE_FRAMES, Math.floor(style.frames || 0)),
    );
    const frames = Math.max(
      1,
      declared || Math.min(MAX_RESOURCE_FRAMES, sourceFrames),
    );
    const resolution = Math.max(
      1,
      Math.min(
        128,
        images[0].width,
        Math.floor(this.renderer.capabilities.maxTextureSize / 6),
        Math.floor(this.renderer.capabilities.maxTextureSize / frames),
      ),
    );
    const canvas = document.createElement("canvas");
    canvas.width = resolution * 6;
    canvas.height = resolution * frames;
    const ctx = canvas.getContext("2d");
    ctx.imageSmoothingEnabled = false;
    for (let face = 0; face < 6; face++) {
      const im = images[face],
        n = frames > 1 ? Math.max(1, Math.floor(im.height / im.width)) : 1;
      for (let f = 0; f < frames; f++) {
        if (n === 1)
          ctx.drawImage(
            im,
            0,
            0,
            im.width,
            im.height,
            face * resolution,
            f * resolution,
            resolution,
            resolution,
          );
        else
          ctx.drawImage(
            im,
            0,
            (f % n) * im.width,
            im.width,
            im.width,
            face * resolution,
            f * resolution,
            resolution,
            resolution,
          );
      }
    }
    const texture = new THREE.CanvasTexture(canvas);
    texture.colorSpace = THREE.SRGBColorSpace;
    texture.magFilter = THREE.NearestFilter;
    texture.minFilter = THREE.NearestFilter;
    texture.generateMipmaps = false;
    texture.wrapS = texture.wrapT = THREE.ClampToEdgeWrapping;
    this.textures.push(texture);
    return {
      texture,
      frames,
      frameTime: Math.max(0.001, Number(style.frameTime) || 0.1),
    };
  }
  updateInstances(mesh, boxes, style, { full = false } = {}) {
    const matrix = new THREE.Matrix4(),
      rotation = new THREE.Quaternion(),
      size = new THREE.Vector3(),
      color = new THREE.Color(),
      dims = mesh.geometry.attributes.mfSize,
      phases = mesh.geometry.attributes.mfMotion;
    const previous = mesh.userData.boxes || [];
    let changed = false;
    mesh.instanceMatrix.clearUpdateRanges();
    mesh.instanceColor?.clearUpdateRanges();
    dims.clearUpdateRanges();
    phases.clearUpdateRanges();
    for (let i = 0; i < boxes.length; i++) {
      const b = boxes[i],
        old = previous[i];
      if (
        !full &&
        old &&
        b.id === old.id &&
        b.min.every((v, a) => v === old.min[a]) &&
        b.size.every((v, a) => v === old.size[a]) &&
        b.tintId === old.tintId &&
        b.motionPhase === old.motionPhase
      )
        continue;
      const position = toThree(
        b.min.map((v, a) => (v + b.size[a] / 2) * this.cellSize),
      );
      size.set(...b.size.map((v) => v * this.cellSize));
      matrix.compose(position, rotation, size);
      mesh.setMatrixAt(i, matrix);
      dims.array.set(b.size, i * 3);
      phases.array[i] = b.motionPhase ?? 0;
      color.setRGB(...style.color, THREE.SRGBColorSpace);
      if (b.tintId) {
        const t = b.tintId;
        color.multiply(
          new THREE.Color(
            ...[0.31, 0.53, 0.79].map((v) => 0.45 + 0.55 * ((t * v) % 1)),
          ),
        );
      }
      mesh.setColorAt(i, color);
      mesh.instanceMatrix.addUpdateRange(i * 16, 16);
      mesh.instanceColor.addUpdateRange(i * 3, 3);
      dims.addUpdateRange(i * 3, 3);
      phases.addUpdateRange(i, 1);
      changed = true;
    }
    mesh.count = boxes.length;
    mesh.userData.boxes = boxes;
    if (changed) {
      mesh.instanceMatrix.needsUpdate = true;
      mesh.instanceColor.needsUpdate = true;
      dims.needsUpdate = true;
      phases.needsUpdate = true;
    }
    mesh.computeBoundingSphere();
    return changed;
  }
  tryIncremental(scene, styles, { mode, scale }) {
    const c = mode === "vrc" ? 0.25 : (scene.cellSize ?? 0.25),
      signature = JSON.stringify(styles),
      visibility = [...this.groupsHidden].sort().join("\0");
    if (
      this.mode !== mode ||
      this.cellSize !== c ||
      this.styleSignature !== signature ||
      this.visibilitySignature !== visibility ||
      this.gridSize !== (scene.size ?? 256)
    )
      return false;
    const groups = new Map();
    for (const b of scene.boxes) {
      if (this.groupsHidden.has(b.group)) continue;
      if (!groups.has(b.styleId)) groups.set(b.styleId, []);
      groups.get(b.styleId).push(b);
    }
    const batches = new Map(this.meshes.map((m) => [m.userData.styleId, m]));
    if (
      [...groups].some(
        ([id, boxes]) =>
          !batches.has(id) ||
          batches.get(id).instanceMatrix.count < boxes.length,
      )
    )
      return false;
    const catalog = new Map(styles.map((m) => [m.id, m]));
    let updated = 0;
    for (const [id, mesh] of batches) {
      const boxes = groups.get(id) || [],
        style = catalog.get(id) || { color: [0.6, 0.6, 0.7] };
      if (this.updateInstances(mesh, boxes, style)) updated++;
    }
    this.boxes = scene.boxes;
    if (this.scale !== scale) {
      this.scale = scale;
      this.content.scale.setScalar(scale);
      this.makeGrid();
    }
    this.renderRevision = scene.revision;
    this.lastUpdate = { incremental: true, changedBatches: updated };
    this.updateSelection();
    return true;
  }
  async load(scene, styles, { mode = this.mode, scale = this.scale } = {}) {
    if (this.tryIncremental(scene, styles, { mode, scale })) return this;
    this.styleSignature = JSON.stringify(styles);
    this.visibilitySignature = [...this.groupsHidden].sort().join("\0");
    this.mode = mode;
    this.scale = scale;
    this.cellSize = mode === "vrc" ? 0.25 : (scene.cellSize ?? 0.25);
    this.boxes = scene.boxes;
    this.styles = styles;
    this.renderRevision = scene.revision;
    this.gridSize = scene.size ?? 256;
    for (const mesh of this.meshes) {
      this.content.remove(mesh);
      mesh.geometry.dispose();
    }
    for (const m of this.materials) m.dispose();
    for (const t of this.textures) t.dispose();
    this.meshes = [];
    this.materials = [];
    this.textures = [];
    this.uniforms = [];
    const groups = new Map(),
      catalog = new Map(styles.map((m) => [m.id, m]));
    for (const b of scene.boxes) {
      if (this.groupsHidden.has(b.group)) continue;
      const key = b.styleId;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(b);
    }
    const unit = new THREE.BoxGeometry(1, 1, 1);
    for (const [id, boxes] of groups) {
      const style = catalog.get(id) || {
          id,
          color: [0.6, 0.6, 0.7],
          roughness: 0.8,
          metallic: 0,
          opacity: 1,
          emission: 0,
          renderMode: "opaque",
        },
        atlas = await this.atlas(style);
      const transparent = style.renderMode === "transparent";
      const parameters = {
        color: 0xffffff,
        vertexColors: false,
        transparent,
        opacity: style.opacity ?? 1,
        depthWrite: !transparent,
        toneMapped: mode !== "vrc",
        side: THREE.FrontSide,
      };
      let material;
      if (mode === "vrc") material = new THREE.MeshBasicMaterial(parameters);
      else
        material = new THREE.MeshStandardMaterial({
          ...parameters,
          metalness: style.metallic ?? 0,
          roughness: style.roughness ?? 0.8,
          emissive: new THREE.Color().setRGB(
            ...style.color,
            THREE.SRGBColorSpace,
          ),
          emissiveIntensity: style.emission ?? 0,
          envMapIntensity: 1.2,
        });
      const geometry = unit.clone();
      const capacity =
        2 ** Math.ceil(Math.log2(Math.max(64, Math.ceil(boxes.length * 1.25))));
      const dimensions = new Float32Array(capacity * 3);
      geometry.setAttribute(
        "mfSize",
        new THREE.InstancedBufferAttribute(dimensions, 3),
      );
      geometry.setAttribute(
        "mfMotion",
        new THREE.InstancedBufferAttribute(new Float32Array(capacity), 1),
      );
      material.onBeforeCompile = (shader) => {
        const uniforms = {
          mfAtlas: { value: atlas?.texture ?? null },
          mfHasAtlas: { value: atlas ? 1 : 0 },
          mfFrames: { value: atlas?.frames ?? 1 },
          mfFrameTime: { value: atlas?.frameTime ?? (style.frameTime || 0.1) },
          mfTime: { value: 0 },
          mfKind: {
            value:
              style.family === "water"
                ? 1
                : style.family === "cloud"
                  ? 2
                  : style.renderMode === "cutout"
                    ? 3
                    : style.uid === "mf:lava"
                      ? 4
                      : 0,
          },
        };
        Object.assign(shader.uniforms, uniforms);
        this.uniforms.push(uniforms);
        shader.vertexShader =
          "attribute vec3 mfSize;attribute float mfMotion;varying float mfPhase; varying vec3 mfLocal; varying vec3 mfDims; varying vec3 mfNormal;\n" +
          shader.vertexShader;
        shader.vertexShader = shader.vertexShader.replace(
          "#include <begin_vertex>",
          "#include <begin_vertex>\nmfLocal=position+0.5;mfDims=mfSize;mfNormal=normal;mfPhase=mfMotion/64.;",
        );
        shader.fragmentShader =
          "uniform sampler2D mfAtlas; uniform float mfHasAtlas,mfFrames,mfFrameTime,mfTime,mfKind; varying vec3 mfLocal,mfDims,mfNormal;varying float mfPhase;\n" +
          shader.fragmentShader;
        shader.fragmentShader = shader.fragmentShader.replace(
          "#include <color_fragment>",
          `#include <color_fragment>
     float axis=abs(mfNormal.x)>.5?0.:(abs(mfNormal.y)>.5?1.:2.);
     vec2 logical=axis<.5?mfLocal.zy*mfDims.zy:(axis<1.5?mfLocal.xz*mfDims.xz:mfLocal.xy*mfDims.xy);
     float face=axis<.5?(mfNormal.x>0.?0.:1.):(axis<1.5?(mfNormal.y>0.?2.:3.):(mfNormal.z>0.?5.:4.));
     if(mfKind==1.)logical+=vec2(mfTime*.12,mfTime*.045)+.03*sin(logical.yx*4.+mfTime);
     vec2 tile=fract(logical);float edge=min(min(tile.x,1.-tile.x),min(tile.y,1.-tile.y));
     if(mfHasAtlas>.5){float frame=mod(floor(mfTime/max(mfFrameTime,.001)+mfPhase*mfFrames+.0001),mfFrames);vec2 uv=vec2((face+clamp(tile.x,.004,.996))/6.,1.-(frame+1.-clamp(tile.y,.004,.996))/mfFrames);vec4 texel=texture2D(mfAtlas,uv);diffuseColor*=texel;}
     else{float checker=mod(floor(tile.x*2.)+floor(tile.y*2.),2.);diffuseColor.rgb*=mix(.88,1.,checker)*mix(.45,1.,step(.025,edge));}
     if(mfKind==2.)diffuseColor.rgb*=.9+.1*sin(mfLocal.x*9.+mfLocal.y*7.+mfLocal.z*8.+mfPhase*6.2831853);
     if(mfKind==3.){float leaf=sin(tile.x*12.)*sin(tile.y*12.);if(mfHasAtlas<.5&&leaf<-.2)discard;if(diffuseColor.a<.5)discard;}
    `,
        );
      };
      material.customProgramCacheKey = () =>
        `mf-${mode}-${style.family}-${!!atlas}-${style.renderMode}`;
      const mesh = new THREE.InstancedMesh(geometry, material, capacity);
      mesh.userData.styleId = id;
      mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
      geometry.attributes.mfSize.setUsage(THREE.DynamicDrawUsage);
      this.updateInstances(mesh, boxes, style, { full: true });
      this.content.add(mesh);
      this.meshes.push(mesh);
      this.materials.push(material);
    }
    unit.dispose();
    this.lastUpdate = {
      incremental: false,
      changedBatches: this.meshes.length,
    };
    this.content.scale.setScalar(scale);
    this.makeGrid();
    this.updateSelection();
    this.renderer.render(this.scene, this.camera);
    return this;
  }
  makeGrid() {
    this.grid.geometry.dispose();
    this.grid.material.dispose();
    this.helpers.remove(this.grid);
    const n = Math.min(this.gridSize ?? 16, 256);
    this.grid = new THREE.GridHelper(
      n * this.cellSize * this.scale,
      Math.min(n, 64),
      0x647788,
      0x2c3e4b,
    );
    this.grid.material.transparent = true;
    this.grid.material.opacity = 0.5;
    this.helpers.add(this.grid);
    this.axes.scale.set(this.scale, this.scale, -this.scale);
  }
  setLayer(layer) {
    this.grid.position.y = layer * this.cellSize * this.scale;
  }
  setScale(scale) {
    this.scale = scale;
    this.content.scale.setScalar(scale);
    this.textPreviewGroup.scale.setScalar(scale);
    this.makeGrid();
    this.updateSelection();
  }
  setTextPreview(boxes, style = {}) {
    this.clearTextPreview();
    const preview = boxes.filter(
      (box) =>
        Array.isArray(box?.min) &&
        Array.isArray(box?.size) &&
        box.size.every((value) => value > 0),
    );
    if (!preview.length) return 0;
    const color = new THREE.Color().setRGB(
      ...(style.color || [0.92, 0.76, 0.43]),
      THREE.SRGBColorSpace,
    );
    if (Number(style.tintId)) {
      const tint = Number(style.tintId);
      color.multiply(
        new THREE.Color(
          ...[0.31, 0.53, 0.79].map(
            (value) => 0.45 + 0.55 * ((tint * value) % 1),
          ),
        ),
      );
    }
    const material = new THREE.MeshBasicMaterial({
      color,
      transparent: true,
      opacity: 0.58,
      depthWrite: false,
      side: THREE.DoubleSide,
    });
    const mesh = new THREE.InstancedMesh(
      new THREE.BoxGeometry(1, 1, 1),
      material,
      preview.length,
    );
    const matrix = new THREE.Matrix4(),
      rotation = new THREE.Quaternion(),
      size = new THREE.Vector3();
    preview.forEach((box, index) => {
      const position = toThree(
        box.min.map(
          (value, axis) => (value + box.size[axis] / 2) * this.cellSize,
        ),
      );
      size.set(...box.size.map((value) => value * this.cellSize));
      matrix.compose(position, rotation, size);
      mesh.setMatrixAt(index, matrix);
    });
    mesh.instanceMatrix.needsUpdate = true;
    mesh.computeBoundingSphere();
    mesh.renderOrder = 20;
    mesh.userData.preview = true;
    this.textPreviewGroup.add(mesh);
    this.textPreviewGroup.scale.setScalar(this.scale);
    this.textPreviewMesh = mesh;
    this.textPreviewBoxes = preview.map(({ min, size }) => ({
      min: [...min],
      size: [...size],
    }));
    return preview.length;
  }
  clearTextPreview() {
    if (!this.textPreviewMesh) return;
    this.textPreviewGroup.remove(this.textPreviewMesh);
    this.textPreviewMesh.geometry.dispose();
    this.textPreviewMesh.material.dispose();
    this.textPreviewMesh = null;
    this.textPreviewBoxes = [];
  }
  bounds() {
    const box = new THREE.Box3();
    for (const b of this.boxes) {
      if (this.groupsHidden.has(b.group)) continue;
      const p = b.min.map(
        (v, i) => (v + b.size[i] / 2) * this.cellSize * this.scale,
      );
      const center = toThree(p),
        extent = new THREE.Vector3(
          ...b.size.map((v) => v * this.cellSize * this.scale),
        );
      box.union(new THREE.Box3().setFromCenterAndSize(center, extent));
    }
    return box;
  }
  view(name = "hero", focus = null) {
    const box = this.bounds();
    let center = box.isEmpty()
      ? new THREE.Vector3(0, 0.6, 0)
      : box.getCenter(new THREE.Vector3());
    let span = box.isEmpty()
      ? 4
      : Math.max(...box.getSize(new THREE.Vector3()).toArray(), 0.3);
    if (focus) {
      center = toThree(focus.map((v) => v * this.cellSize * this.scale));
      span = Math.min(span, 2);
    }
    const directions = {
      hero: [1, 0.75, 1.25],
      front: [0, 0.12, 1],
      top: [0, 1, 0.0001],
      side: [1, 0.1, 0],
      detail: [1, 0.6, 1.1],
    };
    const direction = toThree(directions[name] || directions.hero).normalize();
    this.controls.target.copy(center);
    this.camera.up.set(0, 1, 0);
    if (name === "top") this.camera.up.set(0, 0, 1);
    this.camera.position
      .copy(center)
      .addScaledVector(direction, span * 1.9 + this.cellSize * this.scale);
    this.camera.near = Math.max(0.001, span / 10000);
    this.camera.far = Math.max(500, span * 20);
    this.camera.updateProjectionMatrix();
    this.camera.lookAt(center);
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  }
  projectPoint(p) {
    const v = toThree(p).multiplyScalar(this.scale).project(this.camera),
      r = this.renderer.domElement.getBoundingClientRect();
    return {
      x: r.left + ((v.x + 1) * r.width) / 2,
      y: r.top + ((1 - v.y) * r.height) / 2,
    };
  }
  pick(event, { erase = false, layer = 0 } = {}) {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const mouse = new THREE.Vector2(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      (-(event.clientY - rect.top) / rect.height) * 2 + 1,
    );
    const raycaster = new THREE.Raycaster();
    raycaster.setFromCamera(mouse, this.camera);
    const origin = fromThree(raycaster.ray.origin).map(
        (v) => v / (this.cellSize * this.scale),
      ),
      direction = fromThree(raycaster.ray.direction);
    let best = null;
    for (const b of this.boxes) {
      if (this.groupsHidden.has(b.group)) continue;
      let near = -Infinity,
        far = Infinity,
        axis = 0,
        valid = true;
      for (let a = 0; a < 3; a++) {
        if (Math.abs(direction[a]) < 1e-10) {
          if (origin[a] < b.min[a] || origin[a] > b.min[a] + b.size[a]) {
            valid = false;
            break;
          }
        } else {
          let p = (b.min[a] - origin[a]) / direction[a],
            q = (b.min[a] + b.size[a] - origin[a]) / direction[a];
          if (p > q) [p, q] = [q, p];
          if (p > near) {
            near = p;
            axis = a;
          }
          far = Math.min(far, q);
          if (near > far) {
            valid = false;
            break;
          }
        }
      }
      if (!valid || far < 0 || near < 0) continue;
      if (!best || near < best.t) best = { t: near, box: b, axis };
    }
    if (best) {
      const point = origin.map((v, a) => v + direction[a] * best.t);
      const normal = [0, 0, 0];
      normal[best.axis] = -Math.sign(direction[best.axis]);
      return {
        pos: point.map((v, a) =>
          Math.floor(v + normal[a] * (erase ? -1e-4 : 1e-4)),
        ),
        box: best.box,
        normal,
      };
    }
    if (erase || Math.abs(direction[1]) < 1e-9) return null;
    const t = (layer - origin[1]) / direction[1];
    if (t < 0) return null;
    return {
      pos: origin.map((v, a) => Math.floor(v + direction[a] * t + 1e-7)),
      box: null,
      normal: [0, 1, 0],
    };
  }
  hover(hit, size = [1, 1, 1], erase = false) {
    this.ghost.visible = !!hit;
    if (!hit) return;
    const c = this.cellSize * this.scale;
    this.ghost.scale.set(...size.map((v) => v * c));
    this.ghost.position.copy(
      toThree(hit.pos.map((v, i) => (v + size[i] / 2) * c)),
    );
    this.ghost.material.color.set(erase ? "#e08c82" : "#eac76e");
  }
  updateSelection() {
    const box = new THREE.Box3();
    for (const b of this.boxes) {
      if (!this.selection.has(b.id)) continue;
      const c = this.cellSize * this.scale;
      box.union(
        new THREE.Box3().setFromCenterAndSize(
          toThree(b.min.map((v, a) => (v + b.size[a] / 2) * c)),
          new THREE.Vector3(...b.size.map((v) => v * c)),
        ),
      );
    }
    this.selectedHelper.box.copy(box);
    this.selectedHelper.visible = !box.isEmpty();
  }
  viewShot(shot, width, height) {
    const spec = resolveCamera(
      fitBounds(this.boxes, shot.fit),
      shot,
      width / height,
    );
    const unit = this.cellSize * this.scale;
    if (spec.projection === "orthographic") {
      const o = spec.orthographic;
      this.camera = new THREE.OrthographicCamera(
        o.left * unit,
        o.right * unit,
        o.top * unit,
        o.bottom * unit,
        0.001,
        spec.far * unit,
      );
    } else
      this.camera = new THREE.PerspectiveCamera(
        spec.fov,
        width / height,
        0.001,
        spec.far * unit,
      );
    this.camera.position.copy(toThree(spec.position.map((v) => v * unit)));
    this.camera.up.copy(toThree(spec.up));
    this.controls.object = this.camera;
    this.controls.target.copy(toThree(spec.target.map((v) => v * unit)));
    this.camera.lookAt(this.controls.target);
    this.controls.update();
    return spec;
  }
  async capture(views, width = 768, height = 640, shots = null) {
    const pixelRatio = this.renderer.getPixelRatio(),
      original = this.camera,
      originalState = this.camera.clone(),
      target = this.controls.target.clone();
    this.renderer.setPixelRatio(1);
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    const helpersVisible = this.helpers.visible;
    this.helpers.visible = false;
    const previewVisible = this.textPreviewGroup.visible;
    this.textPreviewGroup.visible = false;
    const images = [];
    try {
      for (const view of views) {
        const shot = shots?.find((s) => s.id === view);
        const spec = shot ? this.viewShot(shot, width, height) : null;
        if (!shot) {
          this.camera = new THREE.PerspectiveCamera(
            original.isPerspectiveCamera ? original.fov : 42,
            width / height,
            0.001,
            5000,
          );
          this.controls.object = this.camera;
          this.view(view);
        }
        for (const uniforms of this.uniforms) uniforms.mfTime.value = 0;
        this.renderer.render(this.scene, this.camera);
        images.push({
          view,
          data: this.renderer.domElement.toDataURL("image/png"),
          camera: spec || {
            position: fromThree(this.camera.position),
            target: fromThree(this.controls.target),
            fov: this.camera.fov,
            coordinateSpace: "render-world",
            projection: "perspective",
          },
        });
      }
    } finally {
      this.camera = original;
      this.camera.copy(originalState);
      this.controls.object = this.camera;
      this.controls.target.copy(target);
      this.controls.update();
      this.helpers.visible = helpersVisible;
      this.textPreviewGroup.visible = previewVisible;
      this.renderer.setPixelRatio(pixelRatio);
      this.resize();
    }
    return images;
  }
}
