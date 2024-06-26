use crate::key::ValkeyKey;
use crate::raw;
use crate::Status;
use crate::ValkeyError;
use crate::ValkeyString;
use std::os::raw::c_long;
use std::ptr;

#[derive(Debug)]
pub struct StreamRecord {
    pub id: raw::RedisModuleStreamID,
    pub fields: Vec<(ValkeyString, ValkeyString)>,
}

#[derive(Debug)]
pub struct StreamIterator<'key> {
    key: &'key ValkeyKey,
}

impl<'key> StreamIterator<'key> {
    pub(crate) fn new(
        key: &ValkeyKey,
        mut from: Option<raw::RedisModuleStreamID>,
        mut to: Option<raw::RedisModuleStreamID>,
        exclusive: bool,
        reverse: bool,
    ) -> Result<StreamIterator, ValkeyError> {
        let mut flags = if exclusive {
            raw::REDISMODULE_STREAM_ITERATOR_EXCLUSIVE as i32
        } else {
            0
        };

        flags |= if reverse {
            raw::REDISMODULE_STREAM_ITERATOR_REVERSE as i32
        } else {
            0
        };

        let res = unsafe {
            raw::RedisModule_StreamIteratorStart.unwrap()(
                key.key_inner,
                flags,
                from.as_mut().map_or(ptr::null_mut(), |v| v),
                to.as_mut().map_or(ptr::null_mut(), |v| v),
            )
        };
        if Status::Ok == res.into() {
            Ok(StreamIterator { key })
        } else {
            Err(ValkeyError::Str("Failed creating stream iterator"))
        }
    }
}

impl<'key> Iterator for StreamIterator<'key> {
    type Item = StreamRecord;

    fn next(&mut self) -> Option<Self::Item> {
        let mut id = raw::RedisModuleStreamID { ms: 0, seq: 0 };
        let mut num_fields: c_long = 0;
        let mut field_name: *mut raw::RedisModuleString = ptr::null_mut();
        let mut field_val: *mut raw::RedisModuleString = ptr::null_mut();
        if Status::Ok
            != unsafe {
                raw::RedisModule_StreamIteratorNextID.unwrap()(
                    self.key.key_inner,
                    &mut id,
                    &mut num_fields,
                )
            }
            .into()
        {
            return None;
        }
        let mut fields = Vec::new();
        while Status::Ok
            == unsafe {
                raw::RedisModule_StreamIteratorNextField.unwrap()(
                    self.key.key_inner,
                    &mut field_name,
                    &mut field_val,
                )
                .into()
            }
        {
            fields.push((
                ValkeyString::from_redis_module_string(ptr::null_mut(), field_name),
                ValkeyString::from_redis_module_string(ptr::null_mut(), field_val),
            ));
        }
        Some(StreamRecord { id, fields })
    }
}

impl<'key> Drop for StreamIterator<'key> {
    fn drop(&mut self) {
        unsafe { raw::RedisModule_StreamIteratorDelete.unwrap()(self.key.key_inner) };
    }
}
