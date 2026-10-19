// Storage layout v1 only. No shader entry point or backend execution is supplied.
struct SpatialHeader {
    extent: vec4<u32>,   // width, height, source vertex x, source vertex y
    grid: vec4<u32>,     // step_cm, layout_version, map_id, reserved
    versions: vec4<u32>, // terrain revision lo/hi, geometry revision lo/hi
    job: vec4<u32>,      // generation lo/hi, bitcast<i32>(z_origin_cm), reserved
}
struct TerrainHeights { values: array<f32>, }
struct TerrainFlags { values: array<u32>, } // bit 0 = sampled water; not a safety proof
struct LocalVertices { values: array<vec4<f32>>, } // local x/y/z meters, w=1
