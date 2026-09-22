import type { UserModel } from "$lib/types"

export type PublicUserData = Pick<
  UserModel,
  | "user_id"
  | "user_type"
  | "created_at"
  | "updated_at"
  | "deleted_at"
  | "name"
  | "slug"
  | "avatar_s3_hash"
  | "website"
  | "user_page"
>

export type OwnUserData = PublicUserData &
  Pick<
    UserModel,
    | "name_changes_left"
    | "last_name_change_added_at"
    | "last_renamed_at"
    | "email"
    | "email_verified_at"
    | "email_validation_info"
    | "email_validation_at"
    | "locales"
    | "real_name"
    | "gender"
    | "birthday"
    | "location"
    | "biography"
  >

export type SanitizedUserData = PublicUserData &
  Partial<Omit<OwnUserData, keyof PublicUserData>>

export function sanitizeUserData(
  user: UserModel,
  isViewingAnotherUser: true
): PublicUserData
export function sanitizeUserData(
  user: UserModel,
  isViewingAnotherUser: false
): OwnUserData
export function sanitizeUserData(
  user: UserModel,
  isViewingAnotherUser: boolean
): SanitizedUserData
export function sanitizeUserData(
  user: UserModel,
  isViewingAnotherUser: boolean
): SanitizedUserData {
  const publicData = publicUserFields(user)
  if (isViewingAnotherUser) return publicData
  return {
    ...publicData,
    name_changes_left: user.name_changes_left,
    last_name_change_added_at: user.last_name_change_added_at,
    last_renamed_at: user.last_renamed_at,
    email: user.email,
    email_verified_at: user.email_verified_at,
    email_validation_info: user.email_validation_info,
    email_validation_at: user.email_validation_at,
    locales: user.locales,
    real_name: user.real_name,
    gender: user.gender,
    birthday: user.birthday,
    location: user.location,
    biography: user.biography
  }
}

function publicUserFields(user: UserModel): PublicUserData {
  return {
    user_id: user.user_id,
    user_type: user.user_type,
    created_at: user.created_at,
    updated_at: user.updated_at,
    deleted_at: user.deleted_at,
    name: user.name,
    slug: user.slug,
    avatar_s3_hash: user.avatar_s3_hash,
    website: user.website,
    user_page: user.user_page
  }
}

export function userProfileFields(user?: SanitizedUserData) {
  return {
    name: user?.name ?? "",
    realName: user?.real_name ?? "",
    email: user?.email ?? "",
    gender: user?.gender ?? "",
    birthday: user?.birthday ?? "",
    location: user?.location ?? "",
    website: user?.website ?? "",
    userPage: user?.user_page ?? "",
    biography: user?.biography ?? "",
    locales: user?.locales?.join(" ") ?? ""
  }
}
