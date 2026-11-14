//! Small ordered bookmark tree. IDs belong to folders; link indices are only
//! used within the current committed profile and are never kept by webpages.
use super::*;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BookmarkItem {
    Link(usize),
    Folder(u32),
}

impl Data {
    pub fn folder(&self, id: u32) -> Option<&BookmarkFolder> {
        self.folders.iter().find(|folder| folder.id == id)
    }

    pub fn items(&self, parent: u32) -> Vec<BookmarkItem> {
        let mut rows: Vec<(u32, BookmarkItem)> = self
            .folders
            .iter()
            .filter(|f| f.parent == parent)
            .map(|f| (f.order, BookmarkItem::Folder(f.id)))
            .chain(
                self.bookmarks
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.folder == parent)
                    .map(|(i, b)| (b.order, BookmarkItem::Link(i))),
            )
            .collect();
        rows.sort_by_key(|(order, _)| *order);
        rows.into_iter().map(|(_, item)| item).collect()
    }

    pub fn item_title(&self, item: BookmarkItem) -> &str {
        match item {
            BookmarkItem::Link(i) => self
                .bookmarks
                .get(i)
                .map(|b| b.title.as_str())
                .unwrap_or(""),
            BookmarkItem::Folder(id) => self.folder(id).map(|f| f.title.as_str()).unwrap_or(""),
        }
    }

    pub fn item_parent(&self, item: BookmarkItem) -> Option<u32> {
        match item {
            BookmarkItem::Link(i) => self.bookmarks.get(i).map(|b| b.folder),
            BookmarkItem::Folder(id) => self.folder(id).map(|f| f.parent),
        }
    }

    pub fn next_order(&self, parent: u32) -> u32 {
        self.folders
            .iter()
            .filter(|f| f.parent == parent)
            .map(|f| f.order)
            .chain(
                self.bookmarks
                    .iter()
                    .filter(|b| b.folder == parent)
                    .map(|b| b.order),
            )
            .max()
            .map_or(0, |v| v.saturating_add(1))
    }

    pub fn folder_path(&self, mut id: u32) -> String {
        let mut names = Vec::new();
        for _ in 0..MAX_FOLDER_DEPTH {
            let Some(folder) = self.folder(id) else {
                break;
            };
            names.push(folder.title.as_str());
            id = folder.parent;
        }
        names.reverse();
        let mut path = String::from("Bookmarks bar");
        for name in names {
            path.push_str(" / ");
            path.push_str(name);
        }
        path
    }

    pub fn add_folder(&mut self, parent: u32, title: String) -> Result<u32, String> {
        if self.folders.len() >= MAX_FOLDERS {
            return Err(format!("The folder limit is {MAX_FOLDERS}."));
        }
        let id = self
            .folders
            .iter()
            .map(|f| f.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("No folder IDs remain.")?;
        self.folders.push(BookmarkFolder {
            id,
            parent,
            title,
            order: self.next_order(parent),
        });
        if let Err(error) = self.validate_folders() {
            self.folders.pop();
            return Err(error);
        }
        Ok(id)
    }

    pub fn move_item(
        &mut self,
        item: BookmarkItem,
        parent: u32,
        position: usize,
    ) -> Result<(), String> {
        if parent != 0 && self.folder(parent).is_none() {
            return Err("The destination folder no longer exists.".into());
        }
        let old_parent = self
            .item_parent(item)
            .ok_or("The bookmark no longer exists.")?;
        let mut staged = self.clone();
        match item {
            BookmarkItem::Link(index) => staged.bookmarks[index].folder = parent,
            BookmarkItem::Folder(id) => {
                staged
                    .folders
                    .iter_mut()
                    .find(|f| f.id == id)
                    .unwrap()
                    .parent = parent
            }
        }
        staged.validate_folders()?;
        let mut rows = self.items(parent);
        rows.retain(|row| *row != item);
        rows.insert(position.min(rows.len()), item);
        staged.set_order(&rows);
        if old_parent != parent {
            let old_rows = staged.items(old_parent);
            staged.set_order(&old_rows);
        }
        *self = staged;
        Ok(())
    }

    fn set_order(&mut self, rows: &[BookmarkItem]) {
        for (order, item) in rows.iter().enumerate() {
            match item {
                BookmarkItem::Link(i) => self.bookmarks[*i].order = order as u32,
                BookmarkItem::Folder(id) => {
                    if let Some(folder) = self.folders.iter_mut().find(|f| f.id == *id) {
                        folder.order = order as u32;
                    }
                }
            }
        }
    }

    pub fn remove_folder(&mut self, id: u32) -> (usize, usize) {
        if id == 0 || self.folder(id).is_none() {
            return (0, 0);
        }
        let mut removed = HashSet::from([id]);
        loop {
            let before = removed.len();
            for folder in &self.folders {
                if removed.contains(&folder.parent) {
                    removed.insert(folder.id);
                }
            }
            if removed.len() == before {
                break;
            }
        }
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| !removed.contains(&b.folder));
        self.folders.retain(|f| !removed.contains(&f.id));
        (before - self.bookmarks.len(), removed.len())
    }

    pub(super) fn validate_folders(&self) -> Result<(), String> {
        if self.folders.len() > MAX_FOLDERS {
            return Err("Too many bookmark folders.".into());
        }
        let mut ids = HashSet::new();
        for folder in &self.folders {
            if folder.id == 0 || !ids.insert(folder.id) {
                return Err("Bookmark folder IDs must be unique and nonzero.".into());
            }
            if folder.title.trim().is_empty()
                || folder.title.len() > MAX_TITLE_BYTES
                || folder.title.chars().any(char::is_control)
            {
                return Err("A folder needs a nonempty name without control characters (maximum 1024 bytes).".into());
            }
        }
        for folder in &self.folders {
            let mut id = folder.id;
            let mut path = HashSet::new();
            while id != 0 {
                if !path.insert(id) {
                    return Err(
                        "A folder cannot be moved into itself or one of its subfolders.".into(),
                    );
                }
                if path.len() > MAX_FOLDER_DEPTH {
                    return Err(format!(
                        "Bookmark folders can be nested up to {MAX_FOLDER_DEPTH} levels."
                    ));
                }
                id = self
                    .folder(id)
                    .ok_or("A bookmark folder references a missing parent.")?
                    .parent;
            }
        }
        if self
            .bookmarks
            .iter()
            .any(|b| b.folder != 0 && !ids.contains(&b.folder))
        {
            return Err("A bookmark references a missing folder.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moves_keep_sibling_order_and_reject_cycles_without_mutation() {
        let mut data = Data::default();
        let a = data.add_folder(0, "A".into()).unwrap();
        let b = data.add_folder(a, "B".into()).unwrap();
        for n in 0..3 {
            data.bookmarks.push(Bookmark {
                title: format!("Link {n}"),
                url: format!("https://example.com/{n}"),
                order: n,
                ..Bookmark::default()
            });
        }
        data.move_item(BookmarkItem::Link(2), 0, 0).unwrap();
        assert_eq!(data.items(0)[0], BookmarkItem::Link(2));
        data.move_item(BookmarkItem::Link(0), b, 0).unwrap();
        assert_eq!(data.items(b), vec![BookmarkItem::Link(0)]);
        let saved = data.clone();
        assert!(data.move_item(BookmarkItem::Folder(a), b, 0).is_err());
        assert_eq!(data, saved);
        assert_eq!(data.remove_folder(a), (1, 2));
        data.validate_folders().unwrap();
        assert_eq!(data.bookmarks.len(), 2);
    }
    #[test]
    fn old_profiles_migrate_and_tree_preferences_round_trip() {
        let old = b"OwnBrowser profile 1\njavascript\ttrue\nimages\ttrue\ndefault_zoom\t100\nhome\t\nbookmark\tFirst\thttps://example.com/1\nbookmark\tSecond\thttps://example.com/2\n";
        let mut data = decode(old).unwrap();
        assert_eq!(
            data.items(0),
            vec![BookmarkItem::Link(0), BookmarkItem::Link(1)]
        );
        let folder = data.add_folder(0, "Research & tools".into()).unwrap();
        data.move_item(BookmarkItem::Link(1), folder, 0).unwrap();
        data.settings.dark_mode = true;
        data.settings.visited_color = Some(0xbbaaff);
        assert_eq!(decode(&encode(&data).unwrap()).unwrap(), data);
        assert_eq!(parse_color(" #01a2FE ").unwrap(), Some(0x01a2fe));
        assert!(parse_color("#1234567").is_err());
    }
}
