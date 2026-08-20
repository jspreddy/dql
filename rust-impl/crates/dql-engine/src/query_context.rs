use dql_models::{QueryIndex, TableMeta};
use dql_parser::Selection;

#[derive(Debug, Clone, PartialEq)]
pub struct LastQueryContext {
    pub table: TableMeta,
    pub index: Option<QueryIndex>,
    pub explicit_projection: bool,
}

impl LastQueryContext {
    pub fn important_columns(&self) -> Vec<String> {
        let mut columns = self.table.primary_key_attributes();
        if let Some(index) = &self.index {
            for attr in index.primary_key_attributes() {
                if !columns.iter().any(|existing| existing == &attr) {
                    columns.push(attr);
                }
            }
        }
        columns
    }

    pub fn preserve_column_order(&self) -> bool {
        self.explicit_projection
    }
}

pub fn query_context_from_read(
    table: TableMeta,
    selection: &Selection,
    index: Option<QueryIndex>,
    follow_up_batch_get: bool,
) -> LastQueryContext {
    LastQueryContext {
        table,
        index,
        explicit_projection: matches!(selection, Selection::Items(_)) || follow_up_batch_get,
    }
}
