// Copyright 2020 Nervos Core Dev
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{ffi::CStr, path};

use crate::{
    db_options::{Cache, OptionsMustOutliveDB},
    ffi, ffi_util, ColumnFamilyDescriptor, Error, Options,
};

#[derive(Clone)]
pub struct FullOptions {
    pub db_opts: Options,
    pub cf_descriptors: Vec<ColumnFamilyDescriptor>,
}

impl FullOptions {
    pub fn load_from_file<P>(
        file: P,
        cache_size: Option<usize>,
        ignore_unknown_options: bool,
    ) -> Result<Self, Error>
    where
        P: AsRef<path::Path>,
    {
        let cpath = ffi_util::to_cpath(
            file,
            "Failed to convert path to CString when load config file.",
        )?;

        let cache = cache_size
            .map(|cache_size| Cache::new_lru_cache(cache_size).expect("create RocksDB cache"));

        unsafe {
            let env = ffi::rocksdb_create_default_env();
            let result = ffi_try!(ffi::rocksdb_options_load_from_file(
                cpath.as_ptr(),
                env,
                ignore_unknown_options,
                cache
                    .as_ref()
                    .map(|c| c.0.inner)
                    .unwrap_or_else(|| ffi::rocksdb_null_cache()),
            ));
            ffi::rocksdb_env_destroy(env);
            let db_opts = result.db_opts;
            let cf_descs = result.cf_descs;
            let cf_descs_size = ffi::rocksdb_column_family_descriptors_count(cf_descs);
            let mut cf_descriptors = Vec::new();
            for index in 0..cf_descs_size {
                let name_raw = ffi::rocksdb_column_family_descriptors_name(cf_descs, index);
                let name_cstr = CStr::from_ptr(name_raw as *const _);
                let name = String::from_utf8_lossy(name_cstr.to_bytes());
                let cf_opts_inner = ffi::rocksdb_column_family_descriptors_options(cf_descs, index);
                let outlive = OptionsMustOutliveDB {
                    row_cache: cache.clone(),
                    ..Default::default()
                };
                let cf_opts = Options {
                    inner: cf_opts_inner,
                    outlive,
                };
                cf_descriptors.push(ColumnFamilyDescriptor::new(name, cf_opts));
            }
            ffi::rocksdb_column_family_descriptors_destroy(cf_descs);

            let outlive = OptionsMustOutliveDB {
                row_cache: cache,
                ..Default::default()
            };

            Ok(Self {
                db_opts: Options {
                    inner: db_opts,
                    outlive,
                },
                cf_descriptors,
            })
        }
    }

    /* This method is used to check those column families which are ignored in the options file,
     * and create column family descriptors with default options for them.
     *
     * For example:
     * If there is only 'cf_A' in the options file, but in fact we need both of 'cf_A' and 'cf_B',
     * after we use `Self::load_from_file(..)`, we will get only two `ColumnFamilyDescriptors`:
     * 'default' and 'cf_A'.
     * Then we can call `full_options.complete_column_families(&["cf_A", "cf_B"])` to add the
     * `ColumnFamilyDescriptor` for "cf_B" with the "default" column family options.
     *
     * Notice:
     * The "default" column family options is not default column family options.
     * They are same only if no "default" column family options was provided in the options file.
     *
     * If `ignore_unknown_column_families` is `false` and there has column families which were
     * provided in the options file but not in the `cf_names`, this method will return an error.
     */
    pub fn complete_column_families(
        &mut self,
        cf_names: &[&str],
        ignore_unknown_column_families: bool,
    ) -> Result<(), Error> {
        let cf_name_default = "default";
        let mut options_default = None;
        for cfd in &self.cf_descriptors {
            if cfd.name == cf_name_default {
                options_default = Some(cfd.options.clone());
                continue;
            }
            if cf_names.iter().any(|cf_name| &cfd.name == cf_name) {
                continue;
            }
            if !ignore_unknown_column_families {
                return Err(Error::new(format!(
                    "an unknown column family named \"{}\"",
                    cfd.name
                )));
            }
        }
        if options_default.is_none() {
            let cf = ColumnFamilyDescriptor::new(cf_name_default, Options::default());
            options_default = Some(cf.options.clone());
            self.cf_descriptors.insert(0, cf);
        }
        let options_default = options_default.unwrap();
        for cf_name in cf_names {
            if cf_name == &cf_name_default {
                return Err(Error::new(format!(
                    "don't name a user-defined column family as \"{}\"",
                    cf_name
                )));
            }
            if self.cf_descriptors.iter().all(|cfd| &cfd.name != cf_name) {
                let cf = ColumnFamilyDescriptor::new(cf_name.to_owned(), options_default.clone());
                self.cf_descriptors.push(cf);
            }
        }
        Ok(())
    }
}
