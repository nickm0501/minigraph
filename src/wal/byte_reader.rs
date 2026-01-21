use super::pgoutput::PgOutputError;

/// A helper for reading multi-byte values from a byte slice with error context.
pub struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    #[cfg(test)]
    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    pub fn read_u8(&mut self, ctx: &'static str) -> Result<u8, PgOutputError> {
        let bytes = self.read_bytes(1, ctx)?;
        Ok(bytes[0])
    }

    pub fn read_u16(&mut self, ctx: &'static str) -> Result<u16, PgOutputError> {
        let bytes = self.read_bytes(2, ctx)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_u32(&mut self, ctx: &'static str) -> Result<u32, PgOutputError> {
        let bytes = self.read_bytes(4, ctx)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_i32(&mut self, ctx: &'static str) -> Result<i32, PgOutputError> {
        let bytes = self.read_bytes(4, ctx)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_cstring(&mut self, ctx: &'static str) -> Result<String, PgOutputError> {
        let start = self.pos;
        let Some(end) = self.data[start..].iter().position(|b| *b == 0) else {
            return Err(PgOutputError::Truncated(ctx));
        };

        let end = start + end;
        let bytes = &self.data[start..end];
        self.pos = end + 1; // consume NUL terminator

        let s = std::str::from_utf8(bytes).map_err(|_| PgOutputError::InvalidUtf8(ctx))?;
        Ok(s.to_string())
    }

    pub fn read_bytes(&mut self, len: usize, ctx: &'static str) -> Result<&'a [u8], PgOutputError> {
        if self.pos + len > self.data.len() {
            return Err(PgOutputError::Truncated(ctx));
        }

        let bytes = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(bytes)
    }
}
