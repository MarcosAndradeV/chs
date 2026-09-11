use types::{self as t, Type as LangType, TypeDatabase};

pub type Type = t::TypeID;

pub struct FieldLayout {
    pub offset: u32,
    pub size: u32,
    pub align: u32,
}

pub struct StructLayout {
    pub size: u32,
    pub align: u32,
    pub fields: Vec<FieldLayout>,
}

impl StructLayout {
    pub fn compute(type_id: t::TypeID, type_db: &TypeDatabase) -> Self {
        let canonical = type_db.resolve(type_id);
        match type_db.get_type(canonical) {
            LangType::Struct {
                fields: Some(fields),
                ..
            } => {
                let mut current_offset: u32 = 0;
                let mut max_align: u32 = 1;
                let mut field_layouts = Vec::new();

                for field in fields {
                    let (f_size, f_align) = type_layout(field.ty, type_db);
                    max_align = std::cmp::max(max_align, f_align);

                    // Align current_offset to f_align
                    current_offset = current_offset.div_ceil(f_align) * f_align;

                    field_layouts.push(FieldLayout {
                        offset: current_offset,
                        size: f_size,
                        align: f_align,
                    });

                    current_offset += f_size;
                }

                // Align total size to max_align
                let total_size = current_offset.div_ceil(max_align) * max_align;

                Self {
                    size: total_size,
                    align: max_align,
                    fields: field_layouts,
                }
            }
            LangType::Tuple(elements) => {
                let mut current_offset: u32 = 0;
                let mut max_align: u32 = 1;
                let mut field_layouts = Vec::new();

                for &elem in elements {
                    let (f_size, f_align) = type_layout(elem, type_db);
                    max_align = std::cmp::max(max_align, f_align);

                    current_offset = current_offset.div_ceil(f_align) * f_align;

                    field_layouts.push(FieldLayout {
                        offset: current_offset,
                        size: f_size,
                        align: f_align,
                    });

                    current_offset += f_size;
                }

                let total_size = current_offset.div_ceil(max_align) * max_align;

                Self {
                    size: total_size,
                    align: max_align,
                    fields: field_layouts,
                }
            }
            t => panic!("Expected fully defined struct or tuple type {t:?}"),
        }
    }
}

pub struct EnumLayout {
    pub size: u32,
    pub align: u32,
    pub payload_offset: u32,
}

impl EnumLayout {
    pub fn compute(type_id: t::TypeID, type_db: &TypeDatabase) -> Self {
        let canonical = type_db.resolve(type_id);
        if let LangType::Enum { repr, variants, .. } = type_db.get_type(canonical) {
            let (tag_size, tag_align) = type_layout(*repr, type_db);
            let mut max_payload_size: u32 = 0;
            let mut max_payload_align: u32 = 1;

            for variant in variants {
                if let Some(payload_ty) = variant.payload {
                    let (p_size, p_align) = type_layout(payload_ty, type_db);
                    max_payload_size = std::cmp::max(max_payload_size, p_size);
                    max_payload_align = std::cmp::max(max_payload_align, p_align);
                }
            }

            let align = std::cmp::max(tag_align, max_payload_align);
            let payload_offset = tag_size.div_ceil(max_payload_align) * max_payload_align;
            let total_size = (payload_offset + max_payload_size).div_ceil(align) * align;

            Self {
                size: total_size,
                align,
                payload_offset,
            }
        } else {
            panic!("Expected enum type");
        }
    }
}

pub fn type_layout(type_id: t::TypeID, type_db: &TypeDatabase) -> (u32, u32) {
    let canonical = type_db.resolve(type_id);
    if canonical == type_db.void() {
        (0, 1)
    } else if canonical == type_db.u8() || canonical == type_db.bool() {
        (1, 1)
    } else if canonical == type_db.int() {
        (4, 4)
    } else if canonical == type_db.usize() {
        (8, 8)
    } else if canonical == type_db.f64() {
        (8, 8)
    } else if canonical == type_db.f32() {
        (4, 4)
    } else if canonical == type_db.string() {
        (16, 8)
    } else {
        match type_db.get_type(canonical) {
            LangType::Bool => (1, 1),
            LangType::Float(bitsize) => match bitsize {
                t::FloatBitSize::_32 => (4, 4),
                t::FloatBitSize::_64 => (8, 8),
            },
            LangType::UntypedFloat => (8, 8),
            LangType::String => (16, 8),
            LangType::UntypedInt => (4, 4),
            LangType::Integer(_, bitsize) => match bitsize {
                t::BitSize::_8 => (1, 1),
                t::BitSize::_16 => (2, 2),
                t::BitSize::_32 => (4, 4),
                t::BitSize::_64 => (8, 8),
            },
            LangType::Pointer(_) | LangType::FnPointer { .. } => (8, 8),
            LangType::Array(inner, size) => {
                let (elem_size, elem_align) = type_layout(*inner, type_db);
                (elem_size * (*size as u64) as u32, elem_align)
            }
            LangType::Slice(_) => (16, 8),
            LangType::DynArray(_) => (24, 8),
            LangType::Struct {
                fields: Some(_), ..
            }
            | LangType::Tuple(..) => {
                let layout = StructLayout::compute(canonical, type_db);
                (layout.size, layout.align)
            }
            LangType::Enum { .. } => {
                let layout = EnumLayout::compute(canonical, type_db);
                (layout.size, layout.align)
            }
            LangType::Distinct { base, .. } => type_layout(*base, type_db),
            LangType::TypeVar(_) => (8, 8),
            LangType::Any(_) => (16, 8),
            LangType::Void | LangType::NoReturn | LangType::Module => (0, 1),
            _ => (0, 1),
        }
    }
}
