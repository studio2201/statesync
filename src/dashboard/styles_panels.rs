//! Dashboard panel and form CSS.
pub const CSS_PANELS: &str = r#".how-sync-card .how-lead {
  font-size: 13px; color: var(--text); margin-bottom: 14px; line-height: 1.5;
}
.how-sync-card .how-lead strong { color: var(--bright); font-weight: 600; }
.how-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}
.how-step {
  background: rgba(0,0,0,0.25);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 12px;
}
.how-num {
  display: inline-flex; align-items: center; justify-content: center;
  width: 22px; height: 22px; border-radius: 999px;
  background: rgba(59, 158, 255, 0.15); color: var(--accent);
  font-size: 11px; font-weight: 700; margin-bottom: 8px;
}
.how-title { color: var(--bright); font-weight: 600; font-size: 13px; margin-bottom: 4px; }
.how-step p { font-size: 12px; color: var(--muted); line-height: 1.45; margin: 0; }
.how-legend {
  display: flex; flex-wrap: wrap; gap: 8px 14px;
  margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border);
  font-size: 11px; color: var(--muted);
}
.how-legend strong { color: var(--bright); font-weight: 600; }
@media (max-width: 900px) {
  .how-grid { grid-template-columns: repeat(2, 1fr); }
}
@media (max-width: 560px) {
  .how-grid { grid-template-columns: 1fr; }
}
.toast {
  position: fixed; bottom: 20px; right: 20px;
  background: var(--card); border: 1px solid var(--border);
  color: var(--bright); padding: 12px 16px; border-radius: 8px;
  font-size: 13px; display: none; z-index: 100; max-width: 420px;
  box-shadow: 0 8px 24px rgba(0,0,0,0.35);
}
.modal {
  position: fixed; inset: 0; background: rgba(0,0,0,0.65);
  display: flex; justify-content: center; align-items: center;
  z-index: 50; padding: 16px;
}
.modal-content {
  background: var(--card); border: 1px solid var(--border);
  width: 100%; max-width: 440px; padding: 20px; border-radius: 10px;
}
.modal-content h2 { color: var(--bright); font-size: 16px; margin-bottom: 16px; }
.form-group { margin-bottom: 14px; }
.form-group label {
  display: block; font-size: 11px; font-weight: 600;
  letter-spacing: 0.04em; text-transform: uppercase;
  color: var(--muted); margin-bottom: 6px;
}
.form-group input, .form-group select, .form-group textarea {
  width: 100%; background: #070a0e; border: 1px solid var(--border);
  color: var(--bright); padding: 9px 10px; border-radius: 6px; font-size: 13px;
}
.form-group input:focus, .form-group textarea:focus {
  outline: none; border-color: var(--accent);
}
.form-hint { font-size: 11px; color: var(--muted); margin-top: 5px; }
.check-row {
  display: flex; align-items: center; gap: 8px;
  font-size: 13px; color: var(--bright); margin: 6px 0;
  text-transform: none; letter-spacing: 0; font-weight: 500;
  cursor: pointer;
}
.check-row input { width: auto; accent-color: var(--accent); }
.modal-actions {
  display: flex; flex-wrap: wrap; justify-content: space-between;
  align-items: center; gap: 10px; margin-top: 18px;
}
.modal-actions .right { display: flex; gap: 8px; margin-left: auto; }
.banner {
  margin-bottom: 16px; padding: 10px 14px; border: 1px solid var(--border);
  border-radius: 8px; background: rgba(0,0,0,0.2); font-size: 12px;
  display: flex; justify-content: space-between; align-items: center; gap: 10px;
}
.footer {
  margin-top: 8px; display: flex; flex-wrap: wrap; justify-content: space-between;
  align-items: center; gap: 10px; font-size: 12px; color: var(--muted);
}
.footer select {
  background: #070a0e; border: 1px solid var(--border); color: var(--text);
  font-size: 12px; padding: 4px 8px; border-radius: 4px;
}
.empty { color: var(--muted); font-size: 13px; }
@media (max-width: 800px) {
  .row-grid { grid-template-columns: 1fr; }
  body { padding: 14px; }
}
"#;
