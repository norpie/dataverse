-- Per-operation results for batch executions

CREATE TABLE IF NOT EXISTS batch_operation_results (
    execution_id INTEGER NOT NULL,
    op_index INTEGER NOT NULL,
    content_id TEXT,
    success INTEGER NOT NULL,
    -- Success fields
    operation_type TEXT,
    result_data TEXT,
    -- Failure fields
    error_status INTEGER,
    error_code TEXT,
    error_message TEXT,
    PRIMARY KEY (execution_id, op_index),
    FOREIGN KEY (execution_id) REFERENCES execution_history(id)
);

CREATE INDEX IF NOT EXISTS idx_batch_op_results_content_id
    ON batch_operation_results (content_id)
    WHERE content_id IS NOT NULL;
