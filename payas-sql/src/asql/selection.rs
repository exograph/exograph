use crate::sql::column::{Column, PhysicalColumn};

pub enum Selection<'a> {
    Seq(Vec<SelectionElement<'a>>),
    Json(Vec<(&'a str, SelectionElement<'a>)>, bool),
}

pub enum SelectionSQL<'a> {
    Single(Column<'a>),
    Seq(Vec<Column<'a>>),
}

impl<'a> Selection<'a> {
    pub fn to_sql(&self) -> SelectionSQL<'a> {
        match self {
            Selection::Seq(seq) => SelectionSQL::Seq(
                seq.iter()
                    .map(|s| match s {
                        SelectionElement::Physical(pc) => Column::Physical(pc),
                        SelectionElement::Compound(_) => todo!(),
                    })
                    .collect(),
            ),
            Selection::Json(seq, multi) => {
                let object_elems = seq
                    .iter()
                    .map(|(k, s)| (k.to_string(), s.to_sql().into()))
                    .collect();

                let json_obj = Column::JsonObject(object_elems);

                if *multi {
                    SelectionSQL::Single(Column::JsonAgg(Box::new(json_obj.into())))
                } else {
                    SelectionSQL::Single(json_obj)
                }
            }
        }
    }
}

pub enum SelectionElement<'a> {
    Physical(&'a PhysicalColumn),
    Compound(Selection<'a>),
}

impl<'a> SelectionElement<'a> {
    pub fn to_sql(&self) -> Column<'a> {
        match self {
            SelectionElement::Physical(pc) => Column::Physical(pc),
            SelectionElement::Compound(s) => match s.to_sql() {
                SelectionSQL::Single(elem) => elem,
                SelectionSQL::Seq(_) => todo!(),
            },
        }
    }
}
