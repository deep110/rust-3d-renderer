// load .obj & .mtl files

use std::{
    borrow::Cow,
    collections::HashMap,
    fmt,
    fs::File,
    io::{self, BufRead, BufReader, Error, Read},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use cgmath::{Vector2, Vector3};

const DEFAULT_OBJECT: &str = "default";
const DEFAULT_GROUP: &str = "default";

/// The model of an a single Material as defined in the .mtl spec.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    pub name: String,

    // Material color and illumination
    pub ka: Option<[f32; 3]>,
    pub kd: Option<[f32; 3]>,
    pub ks: Option<[f32; 3]>,
    pub ke: Option<[f32; 3]>,
    pub km: Option<f32>,
    pub tf: Option<[f32; 3]>,
    pub ns: Option<f32>,
    pub ni: Option<f32>,
    pub tr: Option<f32>,
    pub d: Option<f32>,
    pub illum: Option<i32>,

    // Texture and reflection maps
    pub map_ka: Option<String>,
    pub map_kd: Option<String>,
    pub map_ks: Option<String>,
    pub map_ke: Option<String>,
    pub map_ns: Option<String>,
    pub map_d: Option<String>,
    pub map_bump: Option<String>,
    pub map_refl: Option<String>,
}

impl Material {
    pub fn new(name: String) -> Self {
        Material {
            name,
            ka: None,
            kd: None,
            ks: None,
            ke: None,
            km: None,
            ns: None,
            ni: None,
            tr: None,
            tf: None,
            d: None,
            map_ka: None,
            map_kd: None,
            map_ks: None,
            map_ke: None,
            map_ns: None,
            map_d: None,
            map_bump: None,
            map_refl: None,
            illum: None,
        }
    }
}

/// Indicates type of a missing value
#[derive(Debug)]
pub enum MtlMissingType {
    /// i32
    I32,
    /// f32
    F32,
    /// String
    String,
}

impl fmt::Display for MtlMissingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MtlMissingType::I32 => write!(f, "i32"),
            MtlMissingType::F32 => write!(f, "f32"),
            MtlMissingType::String => write!(f, "String"),
        }
    }
}

/// Errors parsing or loading a .mtl file.
#[derive(Debug)]
pub enum MtlError {
    Io(io::Error),
    /// Given instruction was not in .mtl spec.
    InvalidInstruction(String),
    /// Attempted to parse value, but failed.
    InvalidValue(String),
    /// `newmtl` issued, but no name provided.
    MissingMaterialName,
    /// Instruction requires a value, but that value was not provided.
    MissingValue(MtlMissingType),
}

impl std::error::Error for MtlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MtlError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for MtlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MtlError::Io(err) => write!(f, "I/O error loading a .mtl file: {}", err),
            MtlError::InvalidInstruction(instruction) => {
                write!(f, "Unsupported mtl instruction: {}", instruction)
            }
            MtlError::InvalidValue(val) => {
                write!(f, "Attempted to parse the value '{}' but failed.", val)
            }
            MtlError::MissingMaterialName => write!(f, "newmtl issued, but no name provided."),
            MtlError::MissingValue(ty) => {
                write!(f, "Instruction is missing a value of type '{}'", ty)
            }
        }
    }
}

impl From<io::Error> for MtlError {
    fn from(e: Error) -> Self {
        Self::Io(e)
    }
}

impl<'a> From<Material> for Cow<'a, Material> {
    #[inline]
    fn from(s: Material) -> Cow<'a, Material> {
        Cow::Owned(s)
    }
}

struct Parser<I>(I);

impl<'a, I: Iterator<Item = &'a str>> Parser<I> {
    fn get_vec(&mut self) -> Result<[f32; 3], MtlError> {
        let (x, y, z) = match (self.0.next(), self.0.next(), self.0.next()) {
            (Some(x), Some(y), Some(z)) => (x, y, z),
            other => {
                return Err(MtlError::InvalidValue(format!("{:?}", other)));
            }
        };

        match (x.parse::<f32>(), y.parse::<f32>(), z.parse::<f32>()) {
            (Ok(x), Ok(y), Ok(z)) => Ok([x, y, z]),
            other => Err(MtlError::InvalidValue(format!("{:?}", other))),
        }
    }

    fn get_i32(&mut self) -> Result<i32, MtlError> {
        match self.0.next() {
            Some(v) => FromStr::from_str(v).map_err(|_| MtlError::InvalidValue(v.to_string())),
            None => Err(MtlError::MissingValue(MtlMissingType::I32)),
        }
    }

    fn get_f32(&mut self) -> Result<f32, MtlError> {
        match self.0.next() {
            Some(v) => FromStr::from_str(v).map_err(|_| MtlError::InvalidValue(v.to_string())),
            None => Err(MtlError::MissingValue(MtlMissingType::F32)),
        }
    }

    fn into_string(mut self) -> Result<String, MtlError> {
        match self.0.next() {
            Some(v) => {
                // See note on mtllib parsing in obj.rs for why this is needed/works
                Ok(self.0.fold(v.to_string(), |mut existing, next| {
                    existing.push(' ');
                    existing.push_str(next);
                    existing
                }))
            }
            None => Err(MtlError::MissingValue(MtlMissingType::String)),
        }
    }
}

/// The data represented by the `mtllib` command.
///
/// The material name is replaced by the actual material data when the material
/// libraries are laoded if a match is found.
#[derive(Debug, Clone, PartialEq)]
pub struct Mtl {
    /// Name of the .mtl file.
    pub filename: String,
    /// A list of loaded materials.
    ///
    /// The individual materials are wrapped into an `Arc` to facilitate
    /// referencing this data where these materials are assigned in the
    /// `.obj` file.
    pub materials: Vec<Arc<Material>>,
}

impl Mtl {
    /// Construct a new empty mtl lib with the given file name.
    pub fn new(filename: String) -> Self {
        Mtl {
            filename,
            materials: Vec::new(),
        }
    }

    /// Load the mtl library from the input buffer generated by the given
    /// closure.
    ///
    /// This function overwrites the contents of this library if it has already
    /// been loaded.
    pub fn reload_with<R, F>(
        &mut self,
        obj_dir: impl AsRef<Path>,
        mut resolve: F,
    ) -> Result<&mut Self, MtlError>
    where
        R: BufRead,
        F: FnMut(&Path, &str) -> io::Result<R>,
    {
        self.reload(resolve(obj_dir.as_ref(), &self.filename)?)
    }

    /// Load the mtl library from the given input buffer.
    ///
    /// This function overwrites the contents of this library if it has already
    /// been loaded.
    pub fn reload(&mut self, input: impl Read) -> Result<&mut Self, MtlError> {
        self.materials.clear();
        let input = BufReader::new(input);
        let mut material = None;
        for line in input.lines() {
            let mut parser = match line {
                Ok(ref line) => Parser(line.split_whitespace().filter(|s| !s.is_empty())),
                Err(err) => return Err(MtlError::Io(err)),
            };
            match parser.0.next() {
                Some("newmtl") => {
                    self.materials.extend(material.take().map(Arc::new));
                    material = Some(Material::new(
                        parser
                            .0
                            .next()
                            .ok_or_else(|| MtlError::MissingMaterialName)?
                            .to_string(),
                    ));
                }
                Some("Ka") => {
                    if let Some(ref mut m) = material {
                        m.ka = Some(parser.get_vec()?);
                    }
                }
                Some("Kd") => {
                    if let Some(ref mut m) = material {
                        m.kd = Some(parser.get_vec()?);
                    }
                }
                Some("Ks") => {
                    if let Some(ref mut m) = material {
                        m.ks = Some(parser.get_vec()?);
                    }
                }
                Some("Ke") => {
                    if let Some(ref mut m) = material {
                        m.ke = Some(parser.get_vec()?);
                    }
                }
                Some("Ns") => {
                    if let Some(ref mut m) = material {
                        m.ns = Some(parser.get_f32()?);
                    }
                }
                Some("Ni") => {
                    if let Some(ref mut m) = material {
                        m.ni = Some(parser.get_f32()?);
                    }
                }
                Some("Km") => {
                    if let Some(ref mut m) = material {
                        m.km = Some(parser.get_f32()?);
                    }
                }
                Some("d") => {
                    if let Some(ref mut m) = material {
                        m.d = Some(parser.get_f32()?);
                    }
                }
                Some("Tr") => {
                    if let Some(ref mut m) = material {
                        m.tr = Some(parser.get_f32()?);
                    }
                }
                Some("Tf") => {
                    if let Some(ref mut m) = material {
                        m.tf = Some(parser.get_vec()?);
                    }
                }
                Some("illum") => {
                    if let Some(ref mut m) = material {
                        m.illum = Some(parser.get_i32()?);
                    }
                }
                Some("map_Ka") => {
                    if let Some(ref mut m) = material {
                        m.map_ka = Some(parser.into_string()?);
                    }
                }
                Some("map_Kd") => {
                    if let Some(ref mut m) = material {
                        m.map_kd = Some(parser.into_string()?);
                    }
                }
                Some("map_Ks") => {
                    if let Some(ref mut m) = material {
                        m.map_ks = Some(parser.into_string()?);
                    }
                }
                Some("map_d") => {
                    if let Some(ref mut m) = material {
                        m.map_d = Some(parser.into_string()?);
                    }
                }
                Some("map_refl") | Some("refl") => {
                    if let Some(ref mut m) = material {
                        m.map_refl = Some(parser.into_string()?);
                    }
                }
                Some("map_bump") | Some("map_Bump") | Some("bump") => {
                    if let Some(ref mut m) = material {
                        m.map_bump = Some(parser.into_string()?);
                    }
                }
                Some(other) => {
                    if !other.starts_with('#') {
                        return Err(MtlError::InvalidInstruction(other.to_string()));
                    }
                }
                None => {}
            }
        }

        if let Some(material) = material {
            self.materials.push(Arc::new(material));
        }

        Ok(self)
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
#[derive(Debug, Clone, Hash, PartialEq)]
pub struct SimplePolygon(pub Vec<IndexTuple>);

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

/// Errors parsing or loading a .obj file.
#[derive(Debug)]
pub enum ObjError {
    Io(io::Error),
    /// One of the arguments to `f` is malformed.
    MalformedFaceGroup {
        line_number: usize,
        group: String,
    },
    /// An argument list either has unparsable arguments or is
    /// missing one or more arguments.
    ArgumentListFailure {
        line_number: usize,
        list: String,
    },
    /// `mtllib` command issued, but no name was specified.
    MissingMTLName {
        line_number: usize,
    },
}

impl std::error::Error for ObjError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ObjError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for ObjError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjError::Io(err) => write!(f, "I/O error loading a .obj file: {}", err),
            ObjError::MalformedFaceGroup { line_number, group } => write!(
                f,
                "One of the arguments to `f` is malformed (line: {}, group: {})",
                line_number, group
            ),
            ObjError::ArgumentListFailure { line_number, list } => write!(
                f,
                "An argument list either has unparsable arguments or is missing arguments. (line: {}, list: {})",
                line_number, list
            ),
            ObjError::MissingMTLName { line_number } => write!(
                f,
                "mtllib command issued, but no name was specified. (line: {})",
                line_number
            ),
        }
    }
}

impl From<io::Error> for ObjError {
    fn from(e: Error) -> Self {
        Self::Io(e)
    }
}

/// Error loading individual material libraries.
///
/// The `Vec` items are tuples with first component being the the .mtl file, and
/// the second its corresponding error.
#[derive(Debug)]
pub struct MtlLibsLoadError(pub Vec<(String, MtlError)>);

impl std::error::Error for MtlLibsLoadError {}

impl fmt::Display for MtlLibsLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "One of the material libraries failed to load: {:?}",
            self.0
        )
    }
}

impl From<Vec<(String, MtlError)>> for MtlLibsLoadError {
    fn from(e: Vec<(String, MtlError)>) -> Self {
        MtlLibsLoadError(e)
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

/// The data represented by the `usemtl` command.
///
/// The material name is replaced by the actual material data when the material
/// libraries are laoded if a match is found.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjMaterial {
    /// A reference to a material as a material name.
    Ref(String),
    /// A complete `Material` object loaded from a .mtl file in place of the
    /// material reference.
    Mtl(Arc<Material>),
}

impl ObjMaterial {
    fn name(&self) -> &str {
        match self {
            ObjMaterial::Ref(name) => name.as_str(),
            ObjMaterial::Mtl(material) => material.name.as_str(),
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

/// The data model associated with each `Obj` file.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjData {
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

impl Default for ObjData {
    fn default() -> Self {
        ObjData {
            position: Vec::new(),
            texture: Vec::new(),
            normal: Vec::new(),
            objects: Vec::new(),
            material_libs: Vec::new(),
        }
    }
}

fn normalize(idx: isize, len: usize) -> usize {
    if idx < 0 {
        (len as isize + idx) as usize
    } else {
        idx as usize - 1
    }
}

/// A struct used to store `Obj` data as well as its source directory used to
/// load the referenced .mtl files.
#[derive(Clone, Debug)]
pub struct Obj {
    /// The data associated with this `Obj` file.
    pub data: ObjData,
    /// The path of the parent directory from which this file was read.
    ///
    /// It is not always set since the file may have been read from a `String`.
    pub path: PathBuf,
}

impl Obj {
    /// Load an `Obj` file from the given path with the default load
    /// configuration.
    pub fn load(path: impl AsRef<Path>) -> Result<Obj, ObjError> {
        let f = File::open(path.as_ref())?;
        let data = ObjData::load_buf(&f)?;

        // unwrap is safe since we've read this file before.
        let path = path.as_ref().parent().unwrap().to_owned();

        Ok(Obj { data, path })
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
        let mut errs = Vec::new();
        let mut materials = HashMap::new();

        for mtl_lib in &mut self.data.material_libs {
            match mtl_lib.reload_with(&self.path, &mut resolve) {
                Ok(mtl_lib) => {
                    for m in &mtl_lib.materials {
                        // We don't want to overwrite existing entries because of how the materials
                        // are looked up. From the spec:
                        // "If multiple filenames are specified, the first file
                        //  listed is searched first for the material definition, the second
                        //  file is searched next, and so on."
                        materials
                            .entry(m.name.clone())
                            .or_insert_with(|| Arc::clone(m));
                    }
                }
                Err(err) => {
                    errs.push((mtl_lib.filename.clone(), err));
                }
            }
        }

        // Assign loaded materials to the corresponding objects.
        for object in &mut self.data.objects {
            for group in &mut object.groups {
                if let Some(ref mut mat) = group.material {
                    if let Some(newmat) = materials.get(mat.name()) {
                        *mat = ObjMaterial::Mtl(Arc::clone(newmat));
                    }
                }
            }
        }

        if errs.is_empty() {
            Ok(())
        } else {
            Err(errs.into())
        }
    }
}

impl ObjData {
    fn parse_two(
        line_number: usize,
        n0: Option<&str>,
        n1: Option<&str>,
    ) -> Result<Vector2<f32>, ObjError> {
        let (n0, n1) = match (n0, n1) {
            (Some(n0), Some(n1)) => (n0, n1),
            _ => {
                return Err(ObjError::ArgumentListFailure {
                    line_number,
                    list: format!("{:?} {:?}", n0, n1),
                });
            }
        };
        let normal = match (FromStr::from_str(n0), FromStr::from_str(n1)) {
            (Ok(n0), Ok(n1)) => Vector2::new(n0, n1),
            _ => {
                return Err(ObjError::ArgumentListFailure {
                    line_number,
                    list: format!("{:?} {:?}", n0, n1),
                });
            }
        };
        Ok(normal)
    }

    fn parse_three(
        line_number: usize,
        n0: Option<&str>,
        n1: Option<&str>,
        n2: Option<&str>,
    ) -> Result<Vector3<f32>, ObjError> {
        let (n0, n1, n2) = match (n0, n1, n2) {
            (Some(n0), Some(n1), Some(n2)) => (n0, n1, n2),
            _ => {
                return Err(ObjError::ArgumentListFailure {
                    line_number,
                    list: format!("{:?} {:?} {:?}", n0, n1, n2),
                });
            }
        };
        let normal = match (
            FromStr::from_str(n0),
            FromStr::from_str(n1),
            FromStr::from_str(n2),
        ) {
            (Ok(n0), Ok(n1), Ok(n2)) => Vector3::new(n0, n1, n2),
            _ => {
                return Err(ObjError::ArgumentListFailure {
                    line_number,
                    list: format!("{:?} {:?} {:?}", n0, n1, n2),
                });
            }
        };
        Ok(normal)
    }

    fn parse_group(&self, line_number: usize, group: &str) -> Result<IndexTuple, ObjError> {
        let mut group_split = group.split('/');
        let p: Option<isize> = group_split
            .next()
            .and_then(|idx| FromStr::from_str(idx).ok());
        let t: Option<isize> = group_split.next().and_then(|idx| {
            if idx != "" {
                FromStr::from_str(idx).ok()
            } else {
                None
            }
        });
        let n: Option<isize> = group_split
            .next()
            .and_then(|idx| FromStr::from_str(idx).ok());

        match (p, t, n) {
            (Some(p), t, n) => Ok(IndexTuple(
                normalize(p, self.position.len()),
                t.map(|t| normalize(t, self.texture.len())),
                n.map(|n| normalize(n, self.normal.len())),
            )),
            _ => Err(ObjError::MalformedFaceGroup {
                line_number,
                group: String::from(group),
            }),
        }
    }

    fn parse_face<'b, I>(
        &self,
        line_number: usize,
        groups: &mut I,
    ) -> Result<SimplePolygon, ObjError>
    where
        I: Iterator<Item = &'b str>,
    {
        let mut ret = Vec::with_capacity(4);
        for g in groups {
            let ituple = self.parse_group(line_number, g)?;
            ret.push(ituple);
        }
        Ok(SimplePolygon(ret))
    }

    pub fn load_buf<R: Read>(input: R) -> Result<Self, ObjError> {
        let input = BufReader::new(input);
        let mut dat = ObjData::default();
        let mut object = Object::new(DEFAULT_OBJECT.to_string());
        let mut group: Option<Group> = None;

        for (idx, line) in input.lines().enumerate() {
            let (line, mut words) = match line {
                Ok(ref line) => (
                    line.clone(),
                    line.split_whitespace().filter(|s| !s.is_empty()),
                ),
                Err(err) => {
                    return Err(ObjError::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("failed to readline {}", err),
                    )));
                }
            };
            let first = words.next();

            match first {
                Some("v") => {
                    let (v0, v1, v2) = (words.next(), words.next(), words.next());
                    dat.position.push(Self::parse_three(idx, v0, v1, v2)?);
                }
                Some("vt") => {
                    let (t0, t1) = (words.next(), words.next());
                    dat.texture.push(Self::parse_two(idx, t0, t1)?);
                }
                Some("vn") => {
                    let (n0, n1, n2) = (words.next(), words.next(), words.next());
                    dat.normal.push(Self::parse_three(idx, n0, n1, n2)?);
                }
                Some("f") => {
                    let poly = dat.parse_face(idx, &mut words)?;
                    group = Some(match group {
                        None => {
                            let mut g = Group::new(DEFAULT_GROUP.to_string());
                            g.polys.push(poly);
                            g
                        }
                        Some(mut g) => {
                            g.polys.push(poly);
                            g
                        }
                    });
                }
                Some("o") => {
                    group = match group {
                        Some(val) => {
                            object.groups.push(val);
                            dat.objects.push(object);
                            None
                        }
                        None => None,
                    };
                    object = if line.len() > 2 {
                        let name = line[1..].trim();
                        Object::new(name.to_string())
                    } else {
                        Object::new(DEFAULT_OBJECT.to_string())
                    };
                }
                Some("g") => {
                    object.groups.extend(group.take());

                    if line.len() > 2 {
                        let name = line[2..].trim();
                        group = Some(Group::new(name.to_string()));
                    }
                }
                Some("mtllib") => {
                    // Obj strictly does not allow spaces in filenames.
                    // "mtllib Some File.mtl" is forbidden.
                    // However, everyone does it anyway and if we want to ingest blender-outputted
                    // files, we need to support it. This works by walking word
                    // by word and combining them with a space in between. This may not be a totally
                    // accurate way to do it, but until the parser can be re-worked, this is
                    // good-enough, better-than-before solution.
                    let first_word = words
                        .next()
                        .ok_or_else(|| ObjError::MissingMTLName { line_number: idx })?
                        .to_string();
                    let name = words.fold(first_word, |mut existing, next| {
                        existing.push(' ');
                        existing.push_str(next);
                        existing
                    });
                    dat.material_libs.push(Mtl::new(name));
                }
                Some("usemtl") => {
                    let mut g = group.unwrap_or_else(|| Group::new(DEFAULT_GROUP.to_string()));
                    // we found a new material that was applied to an existing
                    // object. It is treated as a new group.
                    if g.material.is_some() {
                        object.groups.push(g.clone());
                        g.index += 1;
                        g.polys.clear();
                    }
                    g.material = words.next().map(|w| ObjMaterial::Ref(w.to_string()));
                    group = Some(g);
                }
                Some("s") => (),
                Some("l") => (),
                Some(_) => (),
                None => (),
            }
        }

        if let Some(g) = group {
            object.groups.push(g);
        }

        dat.objects.push(object);
        Ok(dat)
    }

    pub fn normalize_vertices(&mut self) {
        let vertices = &mut self.position;
        let values = min_max_vertices(&vertices);

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
