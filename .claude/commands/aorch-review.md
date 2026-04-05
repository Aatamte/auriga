Review agent traces and diagnose issues.

Steps:
1. Use list_classifiers to see what classifiers are registered and their status.
2. Use list_traces to get recent agent sessions.
3. For each trace with high token usage or many turns, use get_trace to inspect the full conversation.
4. Look for patterns: excessive token usage, repeated errors, looping behavior, classifier flags, aborted sessions.
5. Produce a diagnosis report with:
   - Summary of traces reviewed
   - Any anomalies or issues found
   - Token usage analysis (totals, averages, outliers)
   - Classifier findings
   - Recommendations
