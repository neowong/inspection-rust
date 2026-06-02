use rusqlite::types::ToSql;

/// 动态 SQL UPDATE 构建器
///
/// 用于根据 Option 字段动态拼接 UPDATE SET 子句
pub struct DynamicUpdate {
    set_parts: Vec<String>,
    params: Vec<Box<dyn ToSql>>,
    idx: i32,
}

impl DynamicUpdate {
    pub fn new() -> Self {
        Self {
            set_parts: Vec::new(),
            params: Vec::new(),
            idx: 1,
        }
    }

    /// 如果 value 是 Some，则添加一个 SET 字段
    pub fn push_opt<T: ToSql + Clone + 'static>(&mut self, col: &str, value: &Option<T>) {
        if let Some(ref val) = value {
            self.set_parts.push(format!("{} = ?{}", col, self.idx));
            self.params.push(Box::new(val.clone()));
            self.idx += 1;
        }
    }

    /// 直接添加一个 SET 字段（用于加密等特殊情况）
    pub fn push_raw<T: ToSql + 'static>(&mut self, col: &str, value: T) {
        self.set_parts.push(format!("{} = ?{}", col, self.idx));
        self.params.push(Box::new(value));
        self.idx += 1;
    }

    /// 返回当前参数索引（用于构建 WHERE 子句）
    pub fn next_idx(&self) -> i32 {
        self.idx
    }

    /// 返回 (SET 子句列表, 参数列表)
    pub fn finish(self) -> (Vec<String>, Vec<Box<dyn ToSql>>) {
        (self.set_parts, self.params)
    }

    /// 返回 SET 子句是否为空
    pub fn is_empty(&self) -> bool {
        self.set_parts.is_empty()
    }
}
