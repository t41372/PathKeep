//! Responsibilities: distinguish retained-storage failures from native decoder limitations.
//! Not responsible for: interpreting the native transport or authorizing source reads.
use crate::archive::RawReader;
use std::{
    cell::RefCell,
    io::{self, Read},
    rc::Rc,
};

pub(crate) struct RetainedInput {
    pub reader: RawReader,
    pub error: Rc<RefCell<Option<io::Error>>>,
}

impl Read for RetainedInput {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        match self.reader.read(output) {
            Ok(size) => Ok(size),
            Err(error) => {
                *self.error.borrow_mut() = Some(error);
                Err(io::Error::other("archive evidence read failed"))
            }
        }
    }
}
