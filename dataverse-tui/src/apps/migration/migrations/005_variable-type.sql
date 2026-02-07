-- Add declared_type to variables.
-- Default: ValueType::Known(FieldType::Simple(AttributeType::String)) = X'00000E' (bincode v2 standard)
ALTER TABLE variables ADD COLUMN declared_type BLOB NOT NULL DEFAULT X'00000E';
