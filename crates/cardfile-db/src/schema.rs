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
        kind -> Text,
        lock_hash -> Nullable<Text>,
        public_key -> Nullable<Text>,
        secret_key -> Nullable<Text>,
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
        updated_by -> Nullable<Text>,
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
        author_id -> Nullable<Text>,
    }
}

diesel::table! {
    card_shares (card_id, user_id) {
        card_id -> Text,
        user_id -> Text,
        permission -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
        deleted_at -> Nullable<BigInt>,
        rev -> BigInt,
    }
}

diesel::table! {
    card_keys (card_id, user_id) {
        card_id -> Text,
        user_id -> Text,
        wrapped -> Text,
        created_at -> BigInt,
    }
}

diesel::table! {
    api_keys (id) {
        id -> Text,
        user_id -> Text,
        name -> Text,
        prefix -> Text,
        key_hash -> Text,
        allow_private -> Bool,
        read_only -> Bool,
        created_at -> BigInt,
        last_used_at -> Nullable<BigInt>,
        revoked_at -> Nullable<BigInt>,
    }
}

diesel::table! {
    activity (id) {
        id -> Text,
        card_id -> Nullable<Text>,
        actor_id -> Text,
        on_behalf_of -> Nullable<Text>,
        via -> Nullable<Text>,
        action -> Text,
        summary -> Text,
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
        created_at -> BigInt,
        updated_at -> BigInt,
        deleted_at -> Nullable<BigInt>,
        rev -> BigInt,
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
diesel::joinable!(card_shares -> cards (card_id));
diesel::joinable!(card_keys -> cards (card_id));
diesel::joinable!(api_keys -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    app_settings,
    users,
    cards,
    card_revisions,
    tags,
    card_tags,
    blobs,
    card_blobs,
    card_shares,
    card_keys,
    api_keys,
    activity,
);
