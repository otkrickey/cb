public func init_storage<GenericIntoRustString: IntoRustString>(_ db_path: GenericIntoRustString, _ encryption_key: GenericIntoRustString) -> Bool {
    __swift_bridge__$init_storage({ let rustString = db_path.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = encryption_key.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
}
public func migrate_database<GenericIntoRustString: IntoRustString>(_ plain_path: GenericIntoRustString, _ encrypted_path: GenericIntoRustString, _ encryption_key: GenericIntoRustString) -> Bool {
    __swift_bridge__$migrate_database({ let rustString = plain_path.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = encrypted_path.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = encryption_key.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
}
public func save_clipboard_entry<GenericIntoRustString: IntoRustString>(_ content_type: GenericIntoRustString, _ text: GenericIntoRustString, _ source_app: GenericIntoRustString) -> Bool {
    __swift_bridge__$save_clipboard_entry({ let rustString = content_type.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = text.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = source_app.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
}
public func save_clipboard_image<GenericIntoRustString: IntoRustString>(_ image_data: UnsafeBufferPointer<UInt8>, _ source_app: GenericIntoRustString) -> Bool {
    __swift_bridge__$save_clipboard_image(image_data.toFfiSlice(), { let rustString = source_app.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
}
public func save_clipboard_text_v2<GenericIntoRustString: IntoRustString>(_ content_type: GenericIntoRustString, _ preview: GenericIntoRustString, _ text_content_or_empty: GenericIntoRustString, _ blob_sha256_or_empty: GenericIntoRustString, _ byte_size: Int64, _ source_app: GenericIntoRustString) -> Bool {
    __swift_bridge__$save_clipboard_text_v2({ let rustString = content_type.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = preview.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = text_content_or_empty.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = blob_sha256_or_empty.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), byte_size, { let rustString = source_app.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
}
public func save_clipboard_blob_ref<GenericIntoRustString: IntoRustString>(_ content_type: GenericIntoRustString, _ preview: GenericIntoRustString, _ blob_sha256: GenericIntoRustString, _ byte_size: Int64, _ source_app: GenericIntoRustString) -> Bool {
    __swift_bridge__$save_clipboard_blob_ref({ let rustString = content_type.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = preview.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), { let rustString = blob_sha256.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), byte_size, { let rustString = source_app.intoRustString(); rustString.isOwned = false; return rustString.ptr }())
}
public func get_recent_entries(_ limit: Int32) -> RustString {
    RustString(ptr: __swift_bridge__$get_recent_entries(limit))
}
public func delete_entry(_ id: Int64) -> Bool {
    __swift_bridge__$delete_entry(id)
}
public func get_entry_text(_ id: Int64) -> Optional<RustString> {
    { let val = __swift_bridge__$get_entry_text(id); if val != nil { return RustString(ptr: val!) } else { return nil } }()
}
public func get_entry_image(_ id: Int64) -> Optional<RustVec<UInt8>> {
    { let val = __swift_bridge__$get_entry_image(id); if val != nil { return RustVec(ptr: val!) } else { return nil } }()
}
public func get_entry_blob_sha256(_ id: Int64) -> Optional<RustString> {
    { let val = __swift_bridge__$get_entry_blob_sha256(id); if val != nil { return RustString(ptr: val!) } else { return nil } }()
}
public func is_blob_missing(_ id: Int64) -> Bool {
    __swift_bridge__$is_blob_missing(id)
}
public func blob_dir_path() -> Optional<RustString> {
    { let val = __swift_bridge__$blob_dir_path(); if val != nil { return RustString(ptr: val!) } else { return nil } }()
}
public func search_entries<GenericIntoRustString: IntoRustString>(_ query: GenericIntoRustString, _ limit: Int32) -> RustString {
    RustString(ptr: __swift_bridge__$search_entries({ let rustString = query.intoRustString(); rustString.isOwned = false; return rustString.ptr }(), limit))
}
public func get_entries_before(_ before_timestamp: Int64, _ limit: Int32) -> RustString {
    RustString(ptr: __swift_bridge__$get_entries_before(before_timestamp, limit))
}
public func touch_entry(_ id: Int64) -> Bool {
    __swift_bridge__$touch_entry(id)
}
public func cleanup_old_entries(_ max_age_days: Int32) -> Int64 {
    __swift_bridge__$cleanup_old_entries(max_age_days)
}


