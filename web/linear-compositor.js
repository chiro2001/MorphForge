import * as THREE from "three";

// Blend transparent surfaces in linear light, as in a Linear Unity project.
// Encoding each surface to sRGB before blending changes the apparent opacity.
export class LinearCompositor {
  constructor(renderer) {
    this.renderer = renderer;
    this.size = new THREE.Vector2();
    this.target = new THREE.WebGLRenderTarget(1, 1, {
      type: THREE.HalfFloatType,
      samples: Math.min(4, renderer.capabilities.maxSamples),
      depthBuffer: true,
    });
    this.scene = new THREE.Scene();
    this.camera = new THREE.Camera();
    this.material = new THREE.ShaderMaterial({
      uniforms: { image: { value: this.target.texture } },
      vertexShader: "varying vec2 vUv; void main(){ vUv=uv; gl_Position=vec4(position.xy,0.,1.); }",
      fragmentShader: `uniform sampler2D image; varying vec2 vUv;
        void main(){ gl_FragColor=texture2D(image,vUv);
          #include <tonemapping_fragment>
          #include <colorspace_fragment>
        }`,
      depthTest: false,
      depthWrite: false,
      blending: THREE.NoBlending,
    });
    this.quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), this.material);
    this.quad.frustumCulled = false;
    this.scene.add(this.quad);
  }
  render(scene, camera) {
    const renderer = this.renderer;
    renderer.getDrawingBufferSize(this.size);
    if (this.target.width !== this.size.x || this.target.height !== this.size.y)
      this.target.setSize(this.size.x, this.size.y);
    const previous = renderer.getRenderTarget();
    try {
      renderer.setRenderTarget(this.target);
      renderer.render(scene, camera);
      renderer.setRenderTarget(previous);
      renderer.render(this.scene, this.camera);
    } finally {
      renderer.setRenderTarget(previous);
    }
  }
  dispose() {
    this.target.dispose();
    this.quad.geometry.dispose();
    this.material.dispose();
  }
}
