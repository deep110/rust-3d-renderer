// load .obj & .mtl files

use std::{
    borrow::Cow,
    fmt,
    io::{self, BufRead, BufReader, Error, Read},
    path::Path,
    str::FromStr,
    sync::Arc,
};

use cgmath::{Vector2, Vector3};

use super::{Group, MeshData, MeshParser, Object, SimplePolygon, IndexTuple};

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
            MtlError::MissingMaterialName => write!(f, "new mtl issued, but no name provided."),
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

/// Errors parsing or loading a .obj file.
#[derive(Debug)]
pub enum ObjError {
    Io(io::Error),
    Unsupported,
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
            ObjError::Unsupported => write!(f, "unsupported file extension"),
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

fn normalize(idx: isize, len: usize) -> usize {
    if idx < 0 {
        (len as isize + idx) as usize
    } else {
        idx as usize - 1
    }
}

pub struct ObjData {}

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

    fn parse_group(
        &self,
        mesh_data: &mut MeshData,
        line_number: usize,
        group: &str,
    ) -> Result<IndexTuple, ObjError> {
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
                normalize(p, mesh_data.position.len()),
                t.map(|t| normalize(t, mesh_data.texture.len())),
                n.map(|n| normalize(n, mesh_data.normal.len())),
            )),
            _ => Err(ObjError::MalformedFaceGroup {
                line_number,
                group: String::from(group),
            }),
        }
    }

    fn parse_face<'b, I>(
        &self,
        mesh_data: &mut MeshData,
        line_number: usize,
        groups: &mut I,
    ) -> Result<SimplePolygon, ObjError>
    where
        I: Iterator<Item = &'b str>,
    {
        let mut ret = Vec::with_capacity(4);
        for g in groups {
            let ituple = self.parse_group(mesh_data, line_number, g)?;
            ret.push(ituple);
        }
        Ok(ret)
    }

    pub fn load_buf<R: Read>(&mut self, input: R) -> Result<MeshData, ObjError> {
        let input = BufReader::new(input);
        let mut dat = MeshData::default();
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
                    let poly = self.parse_face(&mut dat, idx, &mut words)?;
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
}

impl MeshParser for ObjData {
    fn parse_mesh_data<R: Read>(input: R) -> Result<MeshData, ObjError> {
        let mut obj_data = ObjData{};
        obj_data.load_buf(input)
    }
}
