import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { calculateLineItemTotal } from "../../domain/money.ts";
import { invoiceTotals } from "./moneyService.ts";
import type {
  Invoice,
  InvoiceExportDetail,
  InvoiceLineItem,
} from "./types.ts";

/**
 * Single App-level entry point for MoneyView's `exportInvoice` event.
 *
 * Usage:
 * `<MoneyView on:exportInvoice={(event) => void handleInvoiceExport(event)} />`
 */
export async function handleInvoiceExport(
  input: InvoiceExportDetail | CustomEvent<InvoiceExportDetail>,
): Promise<void> {
  const detail: InvoiceExportDetail = isInvoiceExportEvent(input)
    ? input.detail
    : input;
  assertInvoiceSellerIdentity(detail.invoice);

  if (detail.format === "print") {
    printInvoice(detail.invoice);
    return;
  }

  if (!isTauriDesktop()) {
    throw new Error(
      "Direct PDF export is available in the RudeSync desktop app. Use Print to save a PDF in the browser preview.",
    );
  }

  const { save } = await import("@tauri-apps/plugin-dialog");
  const destinationPath = await save({
    title: `Export ${detail.invoice.number}`,
    defaultPath: `${safePdfBaseName(detail.invoice.number)}.pdf`,
    filters: [{ name: "PDF document", extensions: ["pdf"] }],
  });
  if (!destinationPath) return;

  await invoke("export_invoice_pdf", {
    invoiceId: detail.invoice.id,
    destinationPath: ensurePdfExtension(destinationPath),
  });
}

function assertInvoiceSellerIdentity(invoice: Invoice): void {
  if (!invoice.sellerName.trim()) {
    throw new Error(
      "This invoice needs a seller display or business name before export. Open Settings → Invoice profile, add one, then create a new invoice.",
    );
  }
}

function isInvoiceExportEvent(
  input: InvoiceExportDetail | CustomEvent<InvoiceExportDetail>,
): input is CustomEvent<InvoiceExportDetail> {
  return (
    typeof CustomEvent !== "undefined" &&
    input instanceof CustomEvent
  );
}

function isTauriDesktop(): boolean {
  if (typeof window === "undefined") return false;
  return (
    "__TAURI_INTERNALS__" in window ||
    "__TAURI__" in window
  );
}

function ensurePdfExtension(path: string): string {
  return /\.pdf$/i.test(path) ? path : `${path}.pdf`;
}

function safePdfBaseName(invoiceNumber: string): string {
  let value = invoiceNumber
    .trim()
    .replace(/[<>:"/\\|?*\u0000-\u001F]/g, "-")
    .replace(/[. ]+$/g, "")
    .slice(0, 120);
  if (!value) value = "invoice";
  if (
    /^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)/i.test(value)
  ) {
    value = `invoice-${value}`;
  }
  return value;
}

function printInvoice(invoice: Invoice): void {
  const printWindow = window.open(
    "",
    "_blank",
    "popup=yes,width=920,height=760,resizable=yes,scrollbars=yes",
  );
  if (!printWindow) {
    throw new Error(
      "RudeSync could not open the print preview. Allow pop-ups for this window and try again.",
    );
  }

  const documentHtml = invoicePrintDocument(invoice);
  printWindow.document.open();
  printWindow.document.write(documentHtml);
  printWindow.document.close();
  printWindow.addEventListener(
    "afterprint",
    () => {
      printWindow.close();
    },
    { once: true },
  );

  const runPrint = () => {
    printWindow.focus();
    printWindow.print();
  };
  if (printWindow.document.readyState === "complete") {
    printWindow.setTimeout(runPrint, 80);
  } else {
    printWindow.addEventListener(
      "load",
      () => printWindow.setTimeout(runPrint, 80),
      { once: true },
    );
  }
}

function invoicePrintDocument(invoice: Invoice): string {
  const totals = invoiceTotals(invoice);
  const lineRows = invoice.lineItems
    .map((item) => invoiceLineRow(item, invoice.currency))
    .join("");
  const paymentRows = invoice.payments
    .map(
      (payment) => `
        <tr>
          <td>${html(displayDate(payment.receivedDate))}</td>
          <td>${html(payment.note ?? "Payment received")}</td>
          <td class="number">${html(formatMoney(payment.amountMinor, invoice.currency))}</td>
        </tr>`,
    )
    .join("");
  const milestone = invoice.milestoneLabel
    ? `<span><b>Milestone</b>${html(invoice.milestoneLabel)}</span>`
    : "";
  const discountRow =
    totals.discountMinor > 0
      ? `<div><span>Discount</span><b>-${html(formatMoney(totals.discountMinor, invoice.currency))}</b></div>`
      : "";
  const taxRow =
    totals.taxMinor > 0
      ? `<div><span>Tax${invoice.taxPercentage ? ` (${html(invoice.taxPercentage)}%)` : ""}</span><b>${html(formatMoney(totals.taxMinor, invoice.currency))}</b></div>`
      : "";
  const creditRow =
    totals.creditMinor > 0
      ? `<div class="credit"><span>Credit</span><b>${html(formatMoney(totals.creditMinor, invoice.currency))}</b></div>`
      : "";
  const logoSource = invoiceLogoSource(invoice.sellerLogoPath);
  const brandVisual = logoSource
    ? `<img class="brand-logo" src="${html(logoSource)}" alt="">`
    : `<i class="brand-mark"></i>`;
  const sellerContact = contactLines([
    invoice.sellerEmail,
    invoice.sellerAddress,
  ]);
  const clientContact = contactLines([
    invoice.billToEmail,
    invoice.billToAddress,
  ]);
  const notes = invoice.notes
    ? `<section class="copy-block"><h2>Notes</h2><p>${html(invoice.notes)}</p></section>`
    : "";
  const instructions = invoice.paymentInstructions
    ? `<section class="copy-block"><h2>Payment instructions</h2><p>${html(invoice.paymentInstructions)}</p></section>`
    : "";
  const payments = invoice.payments.length
    ? `<section class="payments">
        <h2>Payment history</h2>
        <table>
          <thead><tr><th>Date</th><th>Reference</th><th class="number">Amount</th></tr></thead>
          <tbody>${paymentRows}</tbody>
        </table>
      </section>`
    : "";

  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width,initial-scale=1">
    <title>${html(invoice.number)} · Invoice</title>
    <style>
      :root { color: #142119; background: #e9eeeb; font-family: "Segoe UI", Arial, sans-serif; }
      * { box-sizing: border-box; }
      body { margin: 0; padding: 28px; background: #e9eeeb; }
      .sheet { width: 210mm; min-height: 297mm; margin: 0 auto; padding: 17mm; background: #fff; box-shadow: 0 8px 36px rgba(16,32,22,.12); }
      .header { display: flex; justify-content: space-between; gap: 28px; padding-bottom: 22px; border-bottom: 2px solid #16934f; }
      .brand { display: flex; align-items: flex-start; gap: 11px; }
      .brand-mark { width: 8px; height: 35px; background: #16934f; border-radius: 2px; }
      .brand-logo { width: 54px; height: 40px; object-fit: contain; object-position: left top; }
      .brand strong { display: block; font-size: 20px; letter-spacing: -.03em; }
      .brand small { display: block; margin-top: 4px; color: #718078; font-size: 10px; text-transform: uppercase; letter-spacing: .12em; }
      .contact-lines span { display: block; margin-top: 3px; color: #718078; font-size: 9px; line-height: 1.45; white-space: pre-line; overflow-wrap: anywhere; }
      .identity { text-align: right; }
      .identity h1 { margin: 0; color: #16934f; font-size: 31px; line-height: 1; letter-spacing: .04em; }
      .identity strong { display: block; margin-top: 8px; font-size: 12px; }
      .status { display: inline-block; margin-top: 7px; padding: 4px 8px; color: #146c3c; font-size: 9px; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; background: #e9f7ef; border-radius: 99px; }
      .meta { display: grid; grid-template-columns: 1fr 1fr 1.5fr; gap: 20px; margin: 25px 0; }
      .meta > span { display: flex; flex-direction: column; min-width: 0; }
      .meta b, .bill-to h2, .copy-block h2, .payments h2 { color: #718078; font-size: 9px; letter-spacing: .1em; text-transform: uppercase; }
      .meta span span { margin-top: 5px; font-size: 11px; font-weight: 650; overflow-wrap: anywhere; }
      .bill-to { margin: 3px 0 25px; padding: 17px 18px; background: #f5f8f6; border-left: 3px solid #16934f; }
      .bill-to h2 { margin: 0 0 7px; }
      .bill-to strong { display: block; font-size: 14px; }
      .bill-to span { display: block; margin-top: 4px; color: #718078; font-size: 10px; }
      table { width: 100%; border-collapse: collapse; table-layout: fixed; }
      th { padding: 9px 8px; color: #fff; font-size: 8px; letter-spacing: .08em; text-align: left; text-transform: uppercase; background: #142119; }
      td { padding: 11px 8px; font-size: 10px; line-height: 1.4; border-bottom: 1px solid #dfe6e1; overflow-wrap: anywhere; vertical-align: top; }
      th.description { width: 48%; }
      th.quantity { width: 12%; }
      th.rate, th.amount { width: 20%; }
      .number { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
      .totals { width: 44%; margin: 22px 0 26px auto; }
      .totals > div { display: flex; justify-content: space-between; gap: 20px; padding: 5px 0; color: #5e6e65; font-size: 10px; }
      .totals b { color: #26372d; font-weight: 650; white-space: nowrap; }
      .totals .total { margin-top: 4px; padding-top: 10px; color: #142119; font-size: 12px; border-top: 1px solid #cfd9d2; }
      .totals .balance { margin-top: 8px; padding: 10px; color: #126f3d; font-size: 12px; background: #eaf8f0; }
      .totals .balance b { color: #126f3d; }
      .totals .credit { color: #126f3d; font-weight: 650; }
      .totals .credit b { color: #126f3d; }
      .copy-block, .payments { margin-top: 24px; break-inside: avoid; }
      .copy-block h2, .payments h2 { margin: 0 0 8px; color: #16934f; }
      .copy-block p { margin: 0; color: #526158; font-size: 10px; line-height: 1.65; white-space: pre-wrap; overflow-wrap: anywhere; }
      .payments table th { color: #526158; background: #edf3ef; }
      .payments table td { padding-top: 8px; padding-bottom: 8px; }
      .footer { margin-top: 34px; padding-top: 12px; color: #829087; font-size: 8px; border-top: 1px solid #dfe6e1; text-align: center; }
      @page { size: A4; margin: 0; }
      @media print {
        body { padding: 0; background: #fff; }
        .sheet { width: 210mm; min-height: 297mm; margin: 0; padding: 17mm; box-shadow: none; }
        thead { display: table-header-group; }
        tr, .bill-to, .totals, .copy-block { break-inside: avoid; }
        .footer { position: running(invoice-footer); }
      }
    </style>
  </head>
  <body>
    <main class="sheet">
      <header class="header">
        <div class="brand">${brandVisual}<div><strong>${html(invoice.sellerName)}</strong><small>Project invoice</small><div class="contact-lines">${sellerContact}</div></div></div>
        <div class="identity"><h1>INVOICE</h1><strong>${html(invoice.number)}</strong><span class="status">${html(statusLabel(invoice.status))}</span></div>
      </header>
      <section class="meta">
        <span><b>Issued</b><span>${html(displayDate(invoice.issueDate))}</span></span>
        <span><b>Due</b><span>${html(displayDate(invoice.dueDate))}</span></span>
        <span><b>Project</b><span>${html(invoice.projectName)}</span></span>
        ${milestone}
      </section>
      <section class="bill-to"><h2>Bill to</h2><strong>${html(invoice.clientName)}</strong><div class="contact-lines">${clientContact}</div></section>
      <table class="items">
        <thead><tr><th class="description">Description</th><th class="quantity number">Qty</th><th class="rate number">Rate</th><th class="amount number">Amount</th></tr></thead>
        <tbody>${lineRows}</tbody>
      </table>
      <section class="totals">
        <div><span>Subtotal</span><b>${html(formatMoney(totals.subtotalMinor, invoice.currency))}</b></div>
        ${discountRow}
        ${taxRow}
        <div class="total"><span>Total</span><b>${html(formatMoney(totals.totalMinor, invoice.currency))}</b></div>
        <div><span>Paid</span><b>${html(formatMoney(totals.paidMinor, invoice.currency))}</b></div>
        <div class="balance"><span>Balance due</span><b>${html(formatMoney(totals.balanceDueMinor, invoice.currency))}</b></div>
        ${creditRow}
      </section>
      ${payments}
      ${notes}
      ${instructions}
      <footer class="footer">${html(invoice.number)} · Generated by RudeSync</footer>
    </main>
  </body>
</html>`;
}

function invoiceLogoSource(path: string | null): string | null {
  if (!path) return null;
  if (/^(?:data:|blob:|https?:)/i.test(path)) return path;
  if (!isTauriDesktop()) return path;
  try {
    return convertFileSrc(path);
  } catch {
    return null;
  }
}

function contactLines(values: Array<string | null>): string {
  return values
    .filter((value): value is string => Boolean(value))
    .map((value) => `<span>${html(value)}</span>`)
    .join("");
}

function invoiceLineRow(
  item: InvoiceLineItem,
  currency: string,
): string {
  let lineTotalMinor = 0;
  try {
    lineTotalMinor = calculateLineItemTotal(
      item.unitPriceMinor,
      item.quantity,
    );
  } catch {
    lineTotalMinor = item.unitPriceMinor;
  }
  return `<tr>
    <td>${html(item.description)}</td>
    <td class="number">${html(item.quantity)}</td>
    <td class="number">${html(formatMoney(item.unitPriceMinor, currency))}</td>
    <td class="number">${html(formatMoney(lineTotalMinor, currency))}</td>
  </tr>`;
}

function formatMoney(minor: number, currency: string): string {
  try {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency,
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }).format(minor / 100);
  } catch {
    return `${currency} ${(minor / 100).toFixed(2)}`;
  }
}

function displayDate(value: string): string {
  const date = new Date(`${value.slice(0, 10)}T12:00:00`);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("en-US", {
    month: "long",
    day: "numeric",
    year: "numeric",
  }).format(date);
}

function statusLabel(status: string): string {
  return status
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function html(value: unknown): string {
  const replacements: Record<string, string> = {
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
  };
  return String(value ?? "").replace(
    /[&<>"']/g,
    (character) => replacements[character],
  );
}
