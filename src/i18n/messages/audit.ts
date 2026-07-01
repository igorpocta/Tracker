/** Audit / Historie změn strings (OpBadge, FilterPills, AuditRow, BeforeAfterDiff). */
export const audit = {
  cs: {
    // Op badge labels
    "audit.op.create": "Vytvořeno",
    "audit.op.update": "Změněno",
    "audit.op.delete": "Smazáno",
    "audit.op.move": "Přesunuto",
    "audit.op.syncTombstone": "Smazáno mimo aplikaci",
    "audit.op.restore": "Obnoveno",
    "audit.op.revert": "Vráceno",
    "audit.op.retry": "Opakováno",
    "audit.op.undo": "Vráceno mazání",
    "audit.op.fallback": "Akce",
    // Filter pills
    "audit.filter.group": "Filtr historie",
    "audit.filter.all": "Vše",
    "audit.filter.delete": "Smazáno",
    "audit.filter.update": "Změněno",
    "audit.filter.failed": "Selhalo",
    // Status
    "audit.status.success": "Úspěšně",
    "audit.status.failed": "Selhalo",
    "audit.status.failedWithError": "Selhalo: {error}",
    // Actions
    "audit.action.retry": "Zkusit znovu",
    "audit.action.retryConfirm": "Opakovat",
    "audit.action.alreadyRestored": "Již obnoveno",
    "audit.action.restore": "Obnovit {where}",
    "audit.action.restoreConfirm": "Obnovit",
    "audit.action.revert": "Vrátit změnu",
    "audit.action.revertConfirm": "Vrátit",
    // Provider locatives (used in restore button label)
    "audit.provider.cloud": "v cloudu",
    "audit.provider.freelo": "ve Freelu",
    "audit.provider.jira": "v Jiře",
    // Before/after diff
    "audit.diff.before": "Před",
    "audit.diff.after": "Po",
    "audit.diff.deleted": "— (smazáno)",
  },
  en: {
    // Op badge labels
    "audit.op.create": "Created",
    "audit.op.update": "Edited",
    "audit.op.delete": "Deleted",
    "audit.op.move": "Moved",
    "audit.op.syncTombstone": "Deleted outside the app",
    "audit.op.restore": "Restored",
    "audit.op.revert": "Reverted",
    "audit.op.retry": "Retried",
    "audit.op.undo": "Deletion reverted",
    "audit.op.fallback": "Action",
    // Filter pills
    "audit.filter.group": "History filter",
    "audit.filter.all": "All",
    "audit.filter.delete": "Deleted",
    "audit.filter.update": "Edited",
    "audit.filter.failed": "Failed",
    // Status
    "audit.status.success": "Success",
    "audit.status.failed": "Failed",
    "audit.status.failedWithError": "Failed: {error}",
    // Actions
    "audit.action.retry": "Try again",
    "audit.action.retryConfirm": "Retry",
    "audit.action.alreadyRestored": "Already restored",
    "audit.action.restore": "Restore {where}",
    "audit.action.restoreConfirm": "Restore",
    "audit.action.revert": "Revert change",
    "audit.action.revertConfirm": "Revert",
    // Provider locatives (used in restore button label)
    "audit.provider.cloud": "in the cloud",
    "audit.provider.freelo": "in Freelo",
    "audit.provider.jira": "in Jira",
    // Before/after diff
    "audit.diff.before": "Before",
    "audit.diff.after": "After",
    "audit.diff.deleted": "— (deleted)",
  },
} as const;
