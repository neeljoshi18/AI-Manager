use crate::model::QueryContext;

/// V1-compatible ACL: public OR intersection of allow-list with user groups.
pub fn acl_allows(ctx: &QueryContext, is_private: bool, allowed_group_ids: &[String]) -> bool {
    if ctx.tenant_id.is_empty() {
        return false;
    }
    if !is_private {
        return true;
    }
    if allowed_group_ids.is_empty() {
        return false;
    }
    allowed_group_ids
        .iter()
        .any(|g| ctx.group_ids.iter().any(|ug| ug == g))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_ok() {
        let ctx = QueryContext {
            tenant_id: "t".into(),
            global_user_id: "u".into(),
            group_ids: vec![],
        };
        assert!(acl_allows(&ctx, false, &[]));
    }

    #[test]
    fn private_requires_group() {
        let ctx = QueryContext {
            tenant_id: "t".into(),
            global_user_id: "u".into(),
            group_ids: vec!["eng".into()],
        };
        assert!(acl_allows(&ctx, true, &["eng".into()]));
        assert!(!acl_allows(&ctx, true, &["sec".into()]));
        assert!(!acl_allows(&ctx, true, &[]));
    }
}
