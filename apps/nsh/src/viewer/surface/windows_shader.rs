use super::*;
use std::{ffi::CString, sync::OnceLock};
use windows::core::{PCSTR, s};

pub(super) struct Pipeline {
    vertex: ID3D11VertexShader,
    fragment: ID3D11PixelShader,
    layout: ID3D11InputLayout,
    raster: ID3D11RasterizerState,
    depth: ID3D11DepthStencilState,
}
struct Shaders {
    vertex: Vec<u8>,
    fragment: Vec<u8>,
}
impl Pipeline {
    pub fn new(device: &ID3D11Device) -> windows::core::Result<Self> {
        // Keep a single lighting source and compile only once, including repeated viewer mounts.
        static SHADERS: OnceLock<Result<Shaders, String>> = OnceLock::new();
        let shaders = SHADERS.get_or_init(compile).as_ref().map_err(failure)?;
        let (vs, ps) = (&shaders.vertex, &shaders.fragment);
        let (mut vertex, mut fragment, mut layout, mut raster, mut depth) =
            (None, None, None, None, None);
        unsafe {
            device.CreateVertexShader(vs, None, Some(&mut vertex))?;
            device.CreatePixelShader(ps, None, Some(&mut fragment))?;
            let fields = [
                (DXGI_FORMAT_R32G32B32_FLOAT, 0, 0),
                (DXGI_FORMAT_R8G8B8A8_SNORM, 0, 12),
                (DXGI_FORMAT_R8G8B8A8_UNORM, 0, 16),
                (DXGI_FORMAT_R32G32B32A32_FLOAT, 1, 0),
                (DXGI_FORMAT_R32G32B32A32_FLOAT, 1, 16),
                (DXGI_FORMAT_R32G32B32A32_FLOAT, 1, 32),
                (DXGI_FORMAT_R32G32B32A32_FLOAT, 1, 48),
            ];
            let elements: Vec<_> = fields
                .into_iter()
                .enumerate()
                .map(|(index, (format, slot, offset))| D3D11_INPUT_ELEMENT_DESC {
                    SemanticName: s!("LOC"),
                    SemanticIndex: index as u32,
                    Format: format,
                    InputSlot: slot,
                    AlignedByteOffset: offset,
                    InputSlotClass: if slot == 0 {
                        D3D11_INPUT_PER_VERTEX_DATA
                    } else {
                        D3D11_INPUT_PER_INSTANCE_DATA
                    },
                    InstanceDataStepRate: slot,
                })
                .collect();
            device.CreateInputLayout(&elements, vs, Some(&mut layout))?;
            device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    FrontCounterClockwise: true.into(),
                    DepthClipEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut raster),
            )?;
            device.CreateDepthStencilState(
                &D3D11_DEPTH_STENCIL_DESC {
                    DepthEnable: true.into(),
                    DepthWriteMask: D3D11_DEPTH_WRITE_MASK_ALL,
                    DepthFunc: D3D11_COMPARISON_LESS_EQUAL,
                    ..Default::default()
                },
                Some(&mut depth),
            )?;
        }
        Ok(Self {
            vertex: vertex.ok_or_else(|| failure("无法创建顶点着色器"))?,
            fragment: fragment.ok_or_else(|| failure("无法创建片段着色器"))?,
            layout: layout.ok_or_else(|| failure("无法创建顶点布局"))?,
            raster: raster.ok_or_else(|| failure("无法创建光栅状态"))?,
            depth: depth.ok_or_else(|| failure("无法创建深度状态"))?,
        })
    }
    pub fn bind(&self, dc: &ID3D11DeviceContext) {
        unsafe {
            dc.IASetInputLayout(&self.layout);
            dc.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            dc.VSSetShader(&self.vertex, None);
            dc.PSSetShader(&self.fragment, None);
            dc.RSSetState(&self.raster);
            dc.OMSetDepthStencilState(&self.depth, 0);
        }
    }
}

fn compile() -> Result<Shaders, String> {
    use naga::{back::hlsl, valid::*};
    let module = naga::front::wgsl::parse_str(include_str!("../board.wgsl"))
        .map_err(|error| error.to_string())?;
    let info = Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .map_err(|error| error.to_string())?;
    let options = hlsl::Options {
        shader_model: hlsl::ShaderModel::V5_0,
        fake_missing_bindings: false,
        binding_map: [(
            naga::ResourceBinding {
                group: 0,
                binding: 0,
            },
            hlsl::BindTarget::default(),
        )]
        .into(),
        ..Default::default()
    };
    let mut hlsl = String::new();
    let reflection = hlsl::Writer::new(&mut hlsl, &options, &hlsl::PipelineOptions::default())
        .write(&module, &info, None)
        .map_err(|error| error.to_string())?;
    let compile_stage = |stage, profile| -> Result<Vec<u8>, String> {
        let index = module
            .entry_points
            .iter()
            .position(|entry| entry.stage == stage)
            .ok_or("缺少三维着色器入口")?;
        let name = reflection.entry_point_names[index]
            .as_ref()
            .map_err(|error| format!("{error:?}"))?;
        let entry = CString::new(name.as_str()).map_err(|error| error.to_string())?;
        let (mut code, mut errors) = (None, None);
        unsafe {
            let result = Fxc::D3DCompile(
                hlsl.as_ptr().cast(),
                hlsl.len(),
                s!("board.wgsl"),
                None,
                None,
                PCSTR(entry.as_ptr().cast()),
                profile,
                Fxc::D3DCOMPILE_ENABLE_STRICTNESS | Fxc::D3DCOMPILE_OPTIMIZATION_LEVEL3,
                0,
                &mut code,
                Some(&mut errors),
            );
            if let Err(error) = result {
                let detail = errors
                    .map(|blob| {
                        String::from_utf8_lossy(std::slice::from_raw_parts(
                            blob.GetBufferPointer().cast(),
                            blob.GetBufferSize(),
                        ))
                        .into_owned()
                    })
                    .unwrap_or_default();
                return Err(format!("三维着色器编译失败：{error} {detail}"));
            }
            let code = code.ok_or("三维着色器未生成字节码")?;
            Ok(
                std::slice::from_raw_parts(code.GetBufferPointer().cast(), code.GetBufferSize())
                    .to_vec(),
            )
        }
    };
    Ok(Shaders {
        vertex: compile_stage(naga::ShaderStage::Vertex, s!("vs_5_0"))?,
        fragment: compile_stage(naga::ShaderStage::Fragment, s!("ps_5_0"))?,
    })
}
