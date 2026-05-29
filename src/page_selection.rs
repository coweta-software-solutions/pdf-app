use std::collections::BTreeSet;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageSelection {
    All,
    Pages(Vec<usize>),
}

impl PageSelection {
    pub fn parse(expr: &str) -> AppResult<Self> {
        let expr = expr.trim();
        if expr.is_empty() {
            return Err(AppError::bad_request("pages cannot be empty"));
        }
        if expr == "all" {
            return Ok(Self::All);
        }

        let mut pages = BTreeSet::new();
        for part in expr.split(',') {
            let part = part.trim();
            if part.is_empty() {
                return Err(AppError::bad_request("pages contains an empty segment"));
            }

            if let Some((start, end)) = part.split_once('-') {
                let start = parse_page(part, start)?;
                let end = parse_page(part, end)?;
                if start > end {
                    return Err(AppError::bad_request("page ranges must be ascending"));
                }
                pages.extend(start..=end);
            } else {
                pages.insert(parse_page(part, part)?);
            }
        }

        if pages.is_empty() {
            Err(AppError::bad_request("pages cannot be empty"))
        } else {
            Ok(Self::Pages(pages.into_iter().collect()))
        }
    }

    pub fn resolve(self, page_count: usize) -> AppResult<Vec<usize>> {
        match self {
            Self::All => Ok((1..=page_count).collect()),
            Self::Pages(pages) if pages.is_empty() => {
                Err(AppError::bad_request("no page numbers specified"))
            }
            Self::Pages(pages) => Ok(pages),
        }
    }
}

fn parse_page(segment: &str, value: &str) -> AppResult<usize> {
    let value = value.trim();
    let page = value
        .parse::<usize>()
        .map_err(|_| AppError::bad_request(format!("invalid page `{segment}`")))?;
    if page == 0 {
        Err(AppError::bad_request("page numbers start at 1"))
    } else {
        Ok(page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_expr_rejects_zero() {
        assert!(PageSelection::parse("0").is_err());
    }

    #[test]
    fn page_expr_rejects_empty_segments() {
        assert!(PageSelection::parse("1,,2").is_err());
    }

    #[test]
    fn page_expr_expands_and_sorts_ranges() {
        assert_eq!(
            PageSelection::parse("3,1-2").unwrap(),
            PageSelection::Pages(vec![1, 2, 3])
        );
    }

    #[test]
    fn page_expr_accepts_all() {
        assert_eq!(PageSelection::parse("all").unwrap(), PageSelection::All);
    }
}
