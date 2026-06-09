#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_invalid_opcode_in_chinese() {
        assert_eq!(
            CpuError::InvalidOpcode(0xd3).to_string(),
            "无效的 LR35902 操作码：0xd3"
        );
    }
}
