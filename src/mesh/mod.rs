//! Parses different file format into Mesh object
//!
//! Right now it only supports wavefront i.e (.obj) file formats
mod wavefront;

use std::{
    ffi::OsStr,
    fmt,
    fs::File,
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
};

use cgmath::{Vector2, Vector3};
use wavefront::*;

/// The data model associated with each `Obj` file.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshData {
    /// Vertex positions.
    pub position: Vec<Vector3<f32>>,
    /// 2D texture coordinates.
    pub texture: Vec<Vector2<f32>>,
    /// A set of normals.
    pub normal: Vec<Vector3<f32>>,
    /// A collection of associated objects indicated by `o`, as well as the
    /// default object at the top level.
    pub objects: Vec<Object>,
    /// The set of all `mtllib` references to .mtl files.
    pub material_libs: Vec<Mtl>,
}

impl Default for MeshData {
    fn default() -> Self {
        MeshData {
            position: Vec::new(),
            texture: Vec::new(),
            normal: Vec::new(),
            objects: Vec::new(),
            material_libs: Vec::new(),
        }
    }
}

impl MeshData {
    pub fn normalize_vertices(&mut self) {
        let vertices = &mut self.position;
        let values = Self::min_max_vertices(&vertices);

        // check if already normalized
        let mut is_valid = true;
        for i in values.iter() {
            if *i <= 1. && *i >= -1. {
                is_valid = is_valid & true;
            } else {
                is_valid = false;
                break;
            }
        }
        if is_valid {
            return;
        }

        let translation = [
            -(values[0] + (values[1] - values[0]) / 2.),
            -(values[2] + (values[3] - values[2]) / 2.),
            -(values[4] + (values[5] - values[4]) / 2.),
        ];

        let scale = [
            1. / ((values[1] - values[0]) / 2.),
            1. / ((values[3] - values[2]) / 2.),
            1. / ((values[5] - values[4]) / 2.),
        ];
        let scale_by = scale
            .iter()
            .min_by(|i, j| i.partial_cmp(j).unwrap())
            .unwrap();

        for vertex in vertices.iter_mut() {
            vertex[0] = (vertex[0] + translation[0]) * scale_by;
            vertex[1] = (vertex[1] + translation[1]) * scale_by;
            vertex[2] = (vertex[2] + translation[2]) * scale_by;
        }
    }

    fn min_max_vertices(vertices: &Vec<Vector3<f32>>) -> [f32; 6] {
        let mut x_min = 0f32;
        let mut x_max = 0f32;
        let mut y_min = 0f32;
        let mut y_max = 0f32;
        let mut z_min = 0f32;
        let mut z_max = 0f32;
        for vertex in vertices {
            let x = vertex.x;
            let y = vertex.y;
            let z = vertex.z;
            if x > x_max {
                x_max = x;
            } else if x < x_min {
                x_min = x;
            }
            if y > y_max {
                y_max = y;
            } else if y < y_min {
                y_min = y;
            }
            if z > z_max {
                z_max = z;
            } else if z < z_min {
                z_min = z;
            }
        }
        return [x_min, x_max, y_min, y_max, z_min, z_max];
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    /// Name of the object assigned by the `o ...` command in the `.obj` file.
    pub name: String,
    /// Groups belonging to this object.
    pub groups: Vec<Group>,
}

impl Object {
    pub fn new(name: String) -> Self {
        Object {
            name,
            groups: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// Name of the group assigned by the `g ...` command in the `.obj` file.
    pub name: String,
    /// An index is used to tell groups apart that share the same name.
    ///
    /// This doesn't appear explicitly in the `.obj` file, but is used here to
    /// simplify groups by limiting them to single materials.
    pub index: usize,
    /// Material assigned to this group via the `usemtl ...` command in the
    /// `.obj` file.
    ///
    /// After material libs are loaded, this will point to the loaded `Material`
    /// struct.
    pub material: Option<ObjMaterial>,
    /// A list of polygons appearing as `f ...` in the `.obj` file.
    pub polys: Vec<SimplePolygon>,
}

impl Group {
    pub fn new(name: String) -> Self {
        Group {
            name,
            index: 0,
            material: None,
            polys: Vec::new(),
        }
    }
}

/// A tuple of position, texture and normal indices assigned to each polygon
/// vertex.
///
/// These appear as `/` separated indices in `.obj` files.
#[derive(Debug, Clone, Copy, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub struct IndexTuple(pub usize, pub Option<usize>, pub Option<usize>);

/// A a simple polygon with arbitrary many vertices.
///
/// Each vertex has an associated tuple of `(position, texture, normal)`
/// indices.
pub type SimplePolygon = Vec<IndexTuple>;

impl std::fmt::Display for IndexTuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0 + 1)?;
        if let Some(idx) = self.1 {
            write!(f, "/{}", idx + 1)?;
        }
        if let Some(idx) = self.2 {
            write!(f, "/{}", idx + 1)?;
        }
        Ok(())
    }
}

trait MeshParser {
    fn parse_mesh_data<R: Read>(input: R) -> Result<MeshData, ObjError>;
}

/// A struct used to store `Mesh` data as well as its source directory used to
/// load the referenced .mtl files.
#[derive(Clone, Debug)]
pub struct MeshLoader {
    /// The data associated with this file format.
    pub data: MeshData,
    /// The path of the parent directory from which this file was read.
    ///
    /// It is not always set since the file may have been read from a `String`.
    pub path: PathBuf,
}

impl MeshLoader {
    /// Load an file from the given path with the default load
    /// configuration.
    pub fn load(path: impl AsRef<Path>) -> Result<MeshLoader, ObjError> {
        println!("{:?}", path.as_ref());
        let f = File::open(path.as_ref())?;

        // unwrap is safe since we've read this file before.
        let path = path.as_ref().parent().unwrap().to_owned();

        let data = match path.extension().and_then(OsStr::to_str) {
            Some("obj") => ObjData::parse_mesh_data(&f)?,
            _ => return Err(ObjError::Unsupported),
        };

        Ok(MeshLoader { data, path })
    }

    /// Loads the .mtl files referenced in the .obj file.
    ///
    /// If it encounters an error for an .mtl, it appends its error to the
    /// returning Vec, and tries the rest.
    pub fn load_mtls(&mut self) -> Result<(), MtlLibsLoadError> {
        self.load_mtls_fn(|obj_dir, mtllib| File::open(&obj_dir.join(mtllib)).map(BufReader::new))
    }

    /// Loads the .mtl files referenced in the .obj file with user provided
    /// loading logic.
    ///
    /// See also [`load_mtls`].
    ///
    /// The provided function must take two arguments:
    ///  - `&Path` - The parent directory of the .obj file
    ///  - `&str`  - The name of the mtllib as listed in the file.
    ///
    /// This function allows loading .mtl files in directories different from
    /// the default .obj directory.
    ///
    /// It must return:
    ///  - Anything that implements [`io::BufRead`] that yields the contents of
    ///    the intended .mtl file.
    ///
    /// [`load_mtls`]: #method.load_mtls
    /// [`io::BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
    pub fn load_mtls_fn<R, F>(&mut self, mut resolve: F) -> Result<(), MtlLibsLoadError>
    where
        R: io::BufRead,
        F: FnMut(&Path, &str) -> io::Result<R>,
    {
        // let mut errs = Vec::new();
        // let mut materials = HashMap::new();

        // for mtl_lib in &mut self.data.material_libs {
        //     match mtl_lib.reload_with(&self.path, &mut resolve) {
        //         Ok(mtl_lib) => {
        //             for m in &mtl_lib.materials {
        //                 // We don't want to overwrite existing entries because of how
        // the materials                 // are looked up. From the spec:
        //                 // "If multiple filenames are specified, the first file
        //                 //  listed is searched first for the material definition, the
        // second                 //  file is searched next, and so on."
        //                 materials
        //                     .entry(m.name.clone())
        //                     .or_insert_with(|| Arc::clone(m));
        //             }
        //         }
        //         Err(err) => {
        //             errs.push((mtl_lib.filename.clone(), err));
        //         }
        //     }
        // }

        // // Assign loaded materials to the corresponding objects.
        // for object in &mut self.data.objects {
        //     for group in &mut object.groups {
        //         if let Some(ref mut mat) = group.material {
        //             if let Some(newmat) = materials.get(mat.name()) {
        //                 *mat = ObjMaterial::Mtl(Arc::clone(newmat));
        //             }
        //         }
        //     }
        // }

        // if errs.is_empty() {
        //     Ok(())
        // } else {
        //     Err(errs.into())
        // }
        Ok(())
    }
}
