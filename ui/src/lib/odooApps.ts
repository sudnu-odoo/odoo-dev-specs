/** Apps offered in the "New database" form: what people call them, and Odoo's name. */
export const ODOO_APPS: { label: string; module: string }[] = [
  { label: "Sales", module: "sale_management" },
  { label: "CRM", module: "crm" },
  { label: "Invoicing", module: "account" },
  { label: "Inventory", module: "stock" },
  { label: "Purchase", module: "purchase" },
  { label: "Manufacturing", module: "mrp" },
  { label: "Project", module: "project" },
  { label: "Timesheets", module: "hr_timesheet" },
  { label: "Employees", module: "hr" },
  { label: "Point of Sale", module: "point_of_sale" },
  { label: "Website", module: "website" },
  { label: "eCommerce", module: "website_sale" },
];

export const ODOO_VERSIONS = ["19.0", "18.0", "17.0"];

/** "sale_management" -> "Sales"; unknown technical names stay as they are. */
export function appLabel(module: string): string {
  return ODOO_APPS.find((a) => a.module === module)?.label ?? module;
}

/** "Sales, Inventory" for a database's apps; "Base only" when none were chosen. */
export function appsText(modules: string[]): string {
  return modules.length ? modules.map(appLabel).join(", ") : "Base only";
}

/** 5242880 -> "5 MB". */
export function sizeText(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  return `${Math.max(1, Math.round(bytes / 1024 ** 2))} MB`;
}

/** "used today", "used 3 days ago", "never used". */
export function usedText(lastUsed: number, now = Date.now()): string {
  if (!lastUsed) return "never used";
  const days = Math.floor((now - lastUsed) / 86_400_000);
  if (days <= 0) return "used today";
  if (days === 1) return "used yesterday";
  return `used ${days} days ago`;
}
