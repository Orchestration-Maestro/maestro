//! Comment part records.
mod comments_part;
mod elements;
pub use comments_part::CommentsPart;
pub use elements::{WmlComment, WmlCommentRangeEnd, WmlCommentRangeStart, WmlCommentReference};
mod comments_extended_part;
pub use comments_extended_part::{CommentsExtended, CommentsExtendedPart};
