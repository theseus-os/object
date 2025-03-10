use core::slice;

use crate::read::{ReadError, ReadRef, Result};
use crate::{pe, LittleEndian as LE};

use super::{ExportTable, ImportTable, RelocationBlockIterator, SectionTable};

/// The table of data directories in a PE file.
#[derive(Debug, Clone, Copy)]
pub struct DataDirectories<'data> {
    entries: &'data [pe::ImageDataDirectory],
}

impl<'data> DataDirectories<'data> {
    /// Parse the data directory table.
    ///
    /// `data` must be the remaining optional data following the
    /// [optional header](pe::ImageOptionalHeader64).  `number` must be from the
    /// [`number_of_rva_and_sizes`](pe::ImageOptionalHeader64::number_of_rva_and_sizes)
    /// field of the optional header.
    pub fn parse(data: &'data [u8], number: u32) -> Result<Self> {
        let entries = data
            .read_slice_at(0, number as usize)
            .read_error("Invalid PE number of RVA and sizes")?;
        Ok(DataDirectories { entries })
    }

    /// The number of data directories.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Iterator over the data directories.
    pub fn iter(&self) -> slice::Iter<'data, pe::ImageDataDirectory> {
        self.entries.iter()
    }

    /// Iterator which gives the directories as well as their index (one of the IMAGE_DIRECTORY_ENTRY_* constants).
    pub fn enumerate(&self) -> core::iter::Enumerate<slice::Iter<'data, pe::ImageDataDirectory>> {
        self.entries.iter().enumerate()
    }

    /// Returns the data directory at the given index.
    ///
    /// Index should be one of the `IMAGE_DIRECTORY_ENTRY_*` constants.
    ///
    /// Returns `None` if the index is larger than the table size,
    /// or if the entry at the index has a zero virtual address.
    pub fn get(&self, index: usize) -> Option<&'data pe::ImageDataDirectory> {
        self.entries
            .get(index)
            .filter(|d| d.virtual_address.get(LE) != 0)
    }

    /// Returns the unparsed export directory.
    ///
    /// `data` must be the entire file data.
    pub fn export_directory<R: ReadRef<'data>>(
        &self,
        data: R,
        sections: &SectionTable<'data>,
    ) -> Result<Option<&'data pe::ImageExportDirectory>> {
        let data_dir = match self.get(pe::IMAGE_DIRECTORY_ENTRY_EXPORT) {
            Some(data_dir) => data_dir,
            None => return Ok(None),
        };
        let export_data = data_dir.data(data, sections)?;
        ExportTable::parse_directory(export_data).map(Some)
    }

    /// Returns the partially parsed export directory.
    ///
    /// `data` must be the entire file data.
    pub fn export_table<R: ReadRef<'data>>(
        &self,
        data: R,
        sections: &SectionTable<'data>,
    ) -> Result<Option<ExportTable<'data>>> {
        let data_dir = match self.get(pe::IMAGE_DIRECTORY_ENTRY_EXPORT) {
            Some(data_dir) => data_dir,
            None => return Ok(None),
        };
        let export_va = data_dir.virtual_address.get(LE);
        let export_data = data_dir.data(data, sections)?;
        ExportTable::parse(export_data, export_va).map(Some)
    }

    /// Returns the partially parsed import directory.
    ///
    /// `data` must be the entire file data.
    pub fn import_table<R: ReadRef<'data>>(
        &self,
        data: R,
        sections: &SectionTable<'data>,
    ) -> Result<Option<ImportTable<'data>>> {
        let data_dir = match self.get(pe::IMAGE_DIRECTORY_ENTRY_IMPORT) {
            Some(data_dir) => data_dir,
            None => return Ok(None),
        };
        let import_va = data_dir.virtual_address.get(LE);
        let (section_data, section_va) = sections
            .pe_data_containing(data, import_va)
            .read_error("Invalid import data dir virtual address")?;
        Ok(Some(ImportTable::new(section_data, section_va, import_va)))
    }

    /// Returns the blocks in the base relocation directory.
    ///
    /// `data` must be the entire file data.
    pub fn relocation_blocks<R: ReadRef<'data>>(
        &self,
        data: R,
        sections: &SectionTable<'data>,
    ) -> Result<Option<RelocationBlockIterator<'data>>> {
        let data_dir = match self.get(pe::IMAGE_DIRECTORY_ENTRY_BASERELOC) {
            Some(data_dir) => data_dir,
            None => return Ok(None),
        };
        let reloc_data = data_dir.data(data, sections)?;
        Ok(Some(RelocationBlockIterator::new(reloc_data)))
    }
}

impl pe::ImageDataDirectory {
    /// Return the virtual address range of this directory entry.
    pub fn address_range(&self) -> (u32, u32) {
        (self.virtual_address.get(LE), self.size.get(LE))
    }

    /// Get the data referenced by this directory entry.
    ///
    /// This function has some limitations:
    /// - It requires that the data is contained in a single section.
    /// - It uses the size field of the directory entry, which is
    /// not desirable for all data directories.
    /// - It uses the `virtual_address` of the directory entry as an address,
    /// which is not valid for `IMAGE_DIRECTORY_ENTRY_SECURITY`.
    pub fn data<'data, R: ReadRef<'data>>(
        &self,
        data: R,
        sections: &SectionTable<'data>,
    ) -> Result<&'data [u8]> {
        sections
            .pe_data_at(data, self.virtual_address.get(LE))
            .read_error("Invalid data dir virtual address")?
            .get(..self.size.get(LE) as usize)
            .read_error("Invalid data dir size")
    }
}
