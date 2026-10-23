use crate::{
    error::{AppError, Result},
    model::BomItem,
};

pub fn search(items: &[BomItem], query: &str) -> Result<Vec<usize>> {
    if query.trim().chars().count() > 100 || query.chars().any(char::is_control) {
        return Err(AppError::new(
            "INVALID_QUERY",
            "物料关键词最多 100 字符，且不能包含控制字符",
        ));
    }
    let query = query.trim().to_lowercase();
    Ok(items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            query.is_empty()
                || [
                    &item.name,
                    &item.designator,
                    &item.footprint,
                    &item.part_number,
                    &item.manufacturer,
                    &item.supplier,
                    &item.supplier_part,
                ]
                .iter()
                .any(|value| value.to_lowercase().contains(&query))
                || item.quantity.to_string().contains(&query)
        })
        .map(|(index, _)| index)
        .collect())
}
