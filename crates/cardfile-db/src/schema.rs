// Diesel table definitions matching `migrations/`. Keep in sync by hand.

diesel::table! {
    app_settings (key) {
        key -> Text,
        value -> Text,
        updated_at -> BigInt,
    }
}

diesel::table! {
    users (id) {
        id -> Text,
        username -> Text,
        display_name -> Text,
        password_hash -> Nullable<Text>,
        is_admin -> Bool,
        settings -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
        deleted_at -> Nullable<BigInt>,
        rev -> BigInt,
    }
}

diesel::table! {
    cards (id) {
        id -> Text,
        owner_id -> Text,
        title -> Text,
        content -> Text,
        plain_text -> Text,
        lock_mode -> Text,
        encrypted -> Nullable<Text>,
        is_pinned -> Bool,
        is_favorite -> Bool,
        sort_key -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
        deleted_at -> Nullable<BigInt>,
        rev -> BigInt,
    }
}

diesel::table! {
    card_revisions (id) {
        id -> Text,
        card_id -> Text,
        title -> Text,
        content -> Text,
        encrypted -> Nullable<Text>,
        created_at -> BigInt,
    }
}

diesel::table! {
    tags (id) {
        id -> Text,
        owner_id -> Text,
        name -> Text,
        color -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
        deleted_at -> Nullable<BigInt>,
        rev -> BigInt,
    }
}

diesel::table! {
    card_tags (card_id, tag_id) {
        card_id -> Text,
        tag_id -> Text,
    }
}

diesel::table! {
    blobs (id) {
        id -> Text,
        mime -> Text,
        size -> BigInt,
        file_name -> Nullable<Text>,
        data -> Binary,
        created_at -> BigInt,
    }
}

diesel::table! {
    card_blobs (card_id, blob_id) {
        card_id -> Text,
        blob_id -> Text,
    }
}

diesel::joinable!(cards -> users (owner_id));
diesel::joinable!(card_revisions -> cards (card_id));
diesel::joinable!(card_tags -> cards (card_id));
diesel::joinable!(card_tags -> tags (tag_id));
diesel::joinable!(card_blobs -> cards (card_id));
diesel::joinable!(card_blobs -> blobs (blob_id));
diesel::joinable!(tags -> users (owner_id));

diesel::allow_tables_to_appear_in_same_query!(
    app_settings,
    users,
    cards,
    card_revisions,
    tags,
    card_tags,
    blobs,
    card_blobs,
);
