//! Malha do mundo na GPU: buffers estáticos por chunk (só reenvia o chunk que mudou) + frustum culling.
//! O batcher do macroquad copiava ~250k vértices pra GPU todo frame.

use crate::world::{CHUNK, CX, CZ, World};
use macroquad::miniquad::{
    Bindings, BlendFactor, BlendState, BlendValue, BufferLayout, BufferSource, BufferType, BufferUsage, Comparison, Equation, PassAction, Pipeline, PipelineParams, ShaderMeta, ShaderSource,
    UniformBlockLayout, UniformDesc, UniformType, UniformsSource, VertexAttribute, VertexFormat,
};
use macroquad::prelude::*;
use macroquad::texture::RenderPass;

#[repr(C)]
struct GpuVertex {
    pos: [f32; 3],
    uv: [f32; 2],
    color: [u8; 4],
}

struct Part {
    bind: Bindings,
    n: i32,
    verts: usize,
}

struct Chunk {
    parts: Vec<Part>,
    lo: Vec3,
    hi: Vec3,
}

pub struct Chunks {
    pipeline: Pipeline,
    tex: Texture2D,
    list: Vec<Chunk>,
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying mediump vec2 uv;
varying lowp vec4 color;
varying lowp vec4 fog;
uniform mat4 Mvp;
uniform vec4 Eye;
uniform vec4 Fog;
void main() {
    gl_Position = Mvp * vec4(position, 1.0);
    color = color0 / 255.0;
    uv = texcoord;
    fog = vec4(Fog.rgb, clamp((distance(position.xz, Eye.xz) - Eye.w) * Fog.a, 0.0, 1.0));
}"#;

const FRAGMENT: &str = r#"#version 100
varying mediump vec2 uv;
varying lowp vec4 color;
varying lowp vec4 fog;
uniform sampler2D Texture;
void main() {
    lowp vec4 c = color * texture2D(Texture, uv);
    gl_FragColor = vec4(mix(c.rgb, fog.rgb, fog.a), c.a);
}"#;

#[repr(C)]
struct Uniforms {
    mvp: Mat4,
    eye: Vec4,
    fog: Vec4,
}

/// Planos do frustum (Gribb-Hartmann) de uma matriz view-projection estilo GL.
pub fn frustum(m: &Mat4) -> [Vec4; 6] {
    let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 + r2, r3 - r2]
}

pub fn aabb_visible(planes: &[Vec4; 6], lo: Vec3, hi: Vec3) -> bool {
    planes.iter().all(|p| {
        let v = vec3(if p.x >= 0.0 { hi.x } else { lo.x }, if p.y >= 0.0 { hi.y } else { lo.y }, if p.z >= 0.0 { hi.z } else { lo.z });
        p.truncate().dot(v) + p.w >= 0.0
    })
}

pub fn sphere_visible(planes: &[Vec4; 6], c: Vec3, r: f32) -> bool {
    planes.iter().all(|p| (p.truncate().dot(c) + p.w) >= -r * p.truncate().length())
}

impl Chunks {
    pub fn new(world: &mut World, tex: &Texture2D) -> Chunks {
        let ctx = unsafe { get_internal_gl() }.quad_context;
        let uniforms = vec![UniformDesc::new("Mvp", UniformType::Mat4), UniformDesc::new("Eye", UniformType::Float4), UniformDesc::new("Fog", UniformType::Float4)];
        let meta = ShaderMeta { uniforms: UniformBlockLayout { uniforms }, images: vec!["Texture".to_string()] };
        let shader = ctx.new_shader(ShaderSource::Glsl { vertex: VERTEX, fragment: FRAGMENT }, meta).expect("shader do mundo");
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[VertexAttribute::new("position", VertexFormat::Float3), VertexAttribute::new("texcoord", VertexFormat::Float2), VertexAttribute::new("color0", VertexFormat::Byte4)],
            shader,
            PipelineParams {
                depth_test: Comparison::LessOrEqual,
                depth_write: true,
                color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha))),
                ..Default::default()
            },
        );
        let mut c = Chunks { pipeline, tex: tex.clone(), list: Vec::new() };
        c.build_all(world);
        c
    }

    pub fn build_all(&mut self, world: &mut World) {
        while self.list.len() < (CX * CZ) as usize {
            self.list.push(Chunk { parts: Vec::new(), lo: Vec3::ZERO, hi: Vec3::ZERO });
        }
        for k in 0..self.list.len() {
            self.rebuild(world, k);
        }
        world.dirty.fill(false);
    }

    /// Remesh de até `max` chunks sujos por frame.
    pub fn update(&mut self, world: &mut World, max: usize) {
        let mut n = 0;
        for k in 0..self.list.len() {
            if world.dirty[k] && n < max {
                self.rebuild(world, k);
                world.dirty[k] = false;
                n += 1;
            }
        }
    }

    fn rebuild(&mut self, world: &World, k: usize) {
        let ctx = unsafe { get_internal_gl() }.quad_context;
        let ch = &mut self.list[k];
        for p in ch.parts.drain(..) {
            ctx.delete_buffer(p.bind.vertex_buffers[0]);
            ctx.delete_buffer(p.bind.index_buffer);
        }
        let (cx, cz) = (k as i32 % CX, k as i32 / CX);
        let (mut ylo, mut yhi) = (f32::MAX, f32::MIN);
        for m in world.build_chunk(cx, cz, &self.tex) {
            let verts: Vec<GpuVertex> = m
                .vertices
                .iter()
                .map(|v| {
                    ylo = ylo.min(v.position.y);
                    yhi = yhi.max(v.position.y);
                    GpuVertex { pos: v.position.to_array(), uv: v.uv.to_array(), color: v.color }
                })
                .collect();
            let vb = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Immutable, BufferSource::slice(&verts));
            let ib = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Immutable, BufferSource::slice(&m.indices));
            let bind = Bindings { vertex_buffers: vec![vb], index_buffer: ib, images: vec![self.tex.raw_miniquad_id()] };
            ch.parts.push(Part { bind, n: m.indices.len() as i32, verts: verts.len() });
        }
        let x0 = (cx * CHUNK) as f32;
        let z0 = (cz * CHUNK) as f32;
        ch.lo = vec3(x0, ylo.min(yhi), z0);
        ch.hi = vec3(x0 + CHUNK as f32, yhi.max(ylo), z0 + CHUNK as f32);
    }

    /// Desenha os chunks visíveis com a matriz `vp` no passe dado (None = tela).
    /// `max_dist`: corta chunks mais longe que isso do olho (plano xz), com neblina na cor `sky` até lá.
    pub fn draw(&self, vp: &Mat4, pass: Option<&RenderPass>, eye: Vec3, max_dist: f32, sky: Color) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let ctx = gl.quad_context;
        match pass {
            Some(p) => ctx.begin_pass(Some(p.raw_miniquad_id()), PassAction::Nothing),
            None => ctx.begin_default_pass(PassAction::Nothing),
        }
        ctx.apply_pipeline(&self.pipeline);
        let fog_start = max_dist * 0.6;
        let inv = if max_dist.is_finite() { 1.0 / (max_dist - fog_start) } else { 0.0 };
        let fog_start = if max_dist.is_finite() { fog_start } else { 0.0 };
        ctx.apply_uniforms(UniformsSource::table(&Uniforms { mvp: *vp, eye: eye.extend(fog_start), fog: vec4(sky.r, sky.g, sky.b, inv) }));
        let planes = frustum(vp);
        for ch in &self.list {
            if ch.parts.is_empty() || !aabb_visible(&planes, ch.lo, ch.hi) {
                continue;
            }
            let c = (ch.lo + ch.hi) * 0.5;
            if vec2(c.x - eye.x, c.z - eye.z).length() > max_dist + CHUNK as f32 * 0.71 {
                continue;
            }
            crate::prof::add(&crate::prof::CHUNKS, 1);
            for p in &ch.parts {
                crate::prof::add(&crate::prof::CALLS, 1);
                crate::prof::add(&crate::prof::VERTS, p.verts);
                ctx.apply_bindings(&p.bind);
                ctx.draw(0, p.n, 1);
            }
        }
        ctx.end_render_pass();
    }
}
