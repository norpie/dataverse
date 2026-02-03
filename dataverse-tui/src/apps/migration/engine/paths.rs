//! Field path parsing for transforms.
//!
//! Provides shared types for parsing and representing field paths used
//! in transforms like `copy` and `format`. Paths support dot notation
//! for traversing lookups and `?` suffix for null-safe traversal.

/// A single segment of a field path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// The field name (without `?` suffix).
    pub name: String,
    /// Whether this segment allows null propagation (`?` suffix).
    pub optional: bool,
}

impl Segment {
    /// Parse a segment from a string.
    ///
    /// If the string ends with `?`, it's marked as optional.
    pub fn parse(s: &str) -> Self {
        if let Some(name) = s.strip_suffix('?') {
            Self {
                name: name.to_owned(),
                optional: true,
            }
        } else {
            Self {
                name: s.to_owned(),
                optional: false,
            }
        }
    }

    /// Get the segment name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Check if this segment is optional (has `?` suffix).
    pub fn is_optional(&self) -> bool {
        self.optional
    }
}

/// A parsed field path with dot-separated segments.
///
/// Examples:
/// - `"name"` → single segment
/// - `"primarycontactid.fullname"` → two segments, first is a lookup
/// - `"primarycontactid?.fullname"` → two segments, first is optional lookup
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPath {
    segments: Vec<Segment>,
}

impl FieldPath {
    /// Parse a path string into segments.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let path = FieldPath::parse("primarycontactid?.fullname");
    /// assert_eq!(path.segments().len(), 2);
    /// assert!(path.segments()[0].is_optional());
    /// ```
    pub fn parse(path: &str) -> Self {
        let segments = path.split('.').map(Segment::parse).collect();
        Self { segments }
    }

    /// Get all segments.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Check if the path is empty.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Get the lookup segments that need `$expand`.
    ///
    /// For a path like `"a.b.c"`, returns `["a", "b"]` since those
    /// are lookups that must be expanded. The final segment `"c"`
    /// is the leaf field to select.
    pub fn lookups(&self) -> &[Segment] {
        if self.segments.len() > 1 {
            &self.segments[..self.segments.len() - 1]
        } else {
            &[]
        }
    }

    /// Get the leaf (final) segment.
    ///
    /// For `"a.b.c"`, returns `"c"`.
    pub fn leaf(&self) -> Option<&Segment> {
        self.segments.last()
    }

    /// Check if this path requires any `$expand`.
    pub fn needs_expand(&self) -> bool {
        self.segments.len() > 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_parse_simple() {
        let seg = Segment::parse("name");
        assert_eq!(seg.name(), "name");
        assert!(!seg.is_optional());
    }

    #[test]
    fn segment_parse_optional() {
        let seg = Segment::parse("primarycontactid?");
        assert_eq!(seg.name(), "primarycontactid");
        assert!(seg.is_optional());
    }

    #[test]
    fn field_path_single_segment() {
        let path = FieldPath::parse("name");
        assert_eq!(path.segments().len(), 1);
        assert_eq!(path.leaf().unwrap().name(), "name");
        assert!(path.lookups().is_empty());
        assert!(!path.needs_expand());
    }

    #[test]
    fn field_path_two_segments() {
        let path = FieldPath::parse("primarycontactid.fullname");
        assert_eq!(path.segments().len(), 2);
        assert_eq!(path.lookups().len(), 1);
        assert_eq!(path.lookups()[0].name(), "primarycontactid");
        assert_eq!(path.leaf().unwrap().name(), "fullname");
        assert!(path.needs_expand());
    }

    #[test]
    fn field_path_three_segments() {
        let path = FieldPath::parse("primarycontactid.parentcustomerid.name");
        assert_eq!(path.segments().len(), 3);
        assert_eq!(path.lookups().len(), 2);
        assert_eq!(path.lookups()[0].name(), "primarycontactid");
        assert_eq!(path.lookups()[1].name(), "parentcustomerid");
        assert_eq!(path.leaf().unwrap().name(), "name");
    }

    #[test]
    fn field_path_with_optional() {
        let path = FieldPath::parse("primarycontactid?.parentcustomerid?.name");
        assert_eq!(path.segments().len(), 3);
        assert!(path.lookups()[0].is_optional());
        assert!(path.lookups()[1].is_optional());
        assert!(!path.leaf().unwrap().is_optional());
    }

    #[test]
    fn field_path_optional_on_leaf() {
        // Edge case: ? on the leaf doesn't really make sense but we parse it
        let path = FieldPath::parse("field?");
        assert_eq!(path.segments().len(), 1);
        assert!(path.leaf().unwrap().is_optional());
    }
}
