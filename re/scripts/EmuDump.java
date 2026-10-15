// EmuDump — inventario y volcado de decompilación para binarios Delphi/Pascal.
// Uso (analyzeHeadless -postScript):
//   EmuDump.java <salidaDir> <modo> [args...]
//   modos:
//     funcs   [max]                      -> TSV de funciones (nombre, addr, tamaño, insns, xrefs, strings)
//     strings                            -> TSV de strings definidos con xrefs
//     decomp <regex> [max]               -> decompila funciones cuyo nombre casa <regex> -> <out>/decomp/
//     top    <n> [regex-exclude]         -> decompila las n funciones más grandes
//     callees <regex>                    -> grafo de llamadas salientes de las que casen <regex>
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

import java.io.File;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.regex.Pattern;

public class EmuDump extends GhidraScript {

    private DecompInterface decomp;

    private PrintWriter w(String name) throws Exception {
        File f = new File(outDir, name);
        if (f.getParentFile() != null) f.getParentFile().mkdirs();
        return new PrintWriter(f, "UTF-8");
    }

    private String outDir = ".";

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) throw new IllegalArgumentException("uso: <salidaDir> <modo> [args...]");
        outDir = args[0];
        new File(outDir).mkdirs();

        decomp = new DecompInterface();
        decomp.openProgram(currentProgram);

        String mode = args[1];
        switch (mode) {
            case "funcs":   funcs(args.length > 2 ? Integer.parseInt(args[2]) : 100000); break;
            case "strings": strings(); break;
            case "decomp":  decompMode(args[2], args.length > 3 ? Integer.parseInt(args[3]) : 60); break;
            case "top":     top(args.length > 2 ? Integer.parseInt(args[2]) : 40,
                                args.length > 3 ? args[3] : null); break;
            case "callees": callees(args[2]); break;
            default: throw new IllegalArgumentException("modo desconocido: " + mode);
        }
        decomp.dispose();
    }

    // ---------------------------------------------------------------- funcs
    private void funcs(int max) throws Exception {
        List<Function> fs = new ArrayList<>();
        FunctionIterator it = currentProgram.getFunctionManager().getFunctions(true);
        while (it.hasNext()) fs.add(it.next());
        fs.sort(Comparator.comparingLong((Function f) -> -size(f)));
        try (PrintWriter p = w("funcs.tsv")) {
            p.println("size\taddr\tname\tspace\tinsns\trefs\tstrings");
            int i = 0;
            for (Function f : fs) {
                if (i++ >= max) break;
                List<String> strs = new ArrayList<>();
                int refs = 0;
                InstructionIterator iit = currentProgram.getListing().getInstructions(f.getBody(), true);
                while (iit.hasNext()) {
                    for (Reference r : currentProgram.getReferenceManager()
                            .getReferencesFrom(iit.next().getAddress())) {
                        refs++;
                        Data d = getDataAt(r.getToAddress());
                        if (d != null && d.getValue() instanceof String) {
                            String v = ((String) d.getValue()).replace("\t", " ").replace("\n", " ");
                            if (v.length() > 2) strs.add(v);
                        }
                    }
                }
                p.printf("%d\t%s\t%s\t%s\t%d\t%d\t%s%n",
                        size(f), f.getEntryPoint(), f.getName(true),
                        f.getBody().getMinAddress().getAddressSpace().getName(),
                        f.getBody().getNumAddresses(), refs, String.join(" | ", strs));
            }
        }
        println("funcs.tsv: " + fs.size() + " funciones");
    }

    // -------------------------------------------------------------- strings
    private void strings() throws Exception {
        try (PrintWriter p = w("strings.tsv")) {
            p.println("addr\tlen\tvalue\txrefcount\txref-addrs");
            currentProgram.getListing().getDefinedData(true).forEach((d) -> {
                if (!(d.getValue() instanceof String)) return;
                String v = d.getValue().toString();
                List<String> xs = new ArrayList<>();
                for (Reference r : currentProgram.getReferenceManager().getReferencesTo(d.getAddress())) xs.add(r.getFromAddress().toString());
                p.printf("%s\t%d\t%s\t%d\t%s%n", d.getAddress(), v.length(),
                        v.replace("\t", " ").replace("\n", "\\n").replace("\r", ""),
                        xs.size(), String.join(" ", xs));
            });
        }
    }

    // --------------------------------------------------------------- decomp
    private void decompMode(String regex, int max) throws Exception {
        Pattern p = Pattern.compile(regex, Pattern.CASE_INSENSITIVE);
        int n = 0;
        for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
            if (n >= max) break;
            String label = f.getName(true) + " " + f.getSignature();
            if (!p.matcher(label).find()) continue;
            emit(f, n++);
        }
        println("decomp: " + n + " funciones -> " + outDir + "/decomp/");
    }

    // ------------------------------------------------------------------ top
    private void top(int n, String exclude) throws Exception {
        Pattern ex = exclude == null ? null : Pattern.compile(exclude, Pattern.CASE_INSENSITIVE);
        List<Function> fs = new ArrayList<>();
        for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
            if (ex != null && ex.matcher(f.getName(true)).find()) continue;
            fs.add(f);
        }
        fs.sort(Comparator.comparingLong((Function f) -> -size(f)));
        int i = 0;
        for (Function f : fs) { if (i >= n) break; emit(f, i++); }
        println("top: " + i + " funciones -> " + outDir + "/decomp/");
    }

    // -------------------------------------------------------------- callees
    private void callees(String regex) throws Exception {
        Pattern p = Pattern.compile(regex, Pattern.CASE_INSENSITIVE);
        try (PrintWriter o = w("callees.tsv")) {
            o.println("caller\tcallee\tcallee-addr");
            for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
                if (!p.matcher(f.getName(true)).find()) continue;
                for (Function g : f.getCalledFunctions(monitor)) {
                    o.printf("%s\t%s\t%s%n", f.getName(true), g.getName(true), g.getEntryPoint());
                }
            }
        }
    }

    // ----------------------------------------------------------------- util
    private long size(Function f) {
        long n = 0;
        InstructionIterator it = currentProgram.getListing().getInstructions(f.getBody(), true);
        while (it.hasNext()) { it.next(); n++; }
        return n;
    }

    private void emit(Function f, int idx) {
        new File(outDir, "decomp").mkdirs();
        DecompileResults r = decomp.decompileFunction(f, 2400, monitor);
        String name = f.getName(true).replaceAll("[^A-Za-z0-9_]", "_");
        String ext = r.decompileCompleted() ? "c" : "err";
        File out = new File(outDir + "/decomp", String.format("%03d_%s_%s.%s", idx, f.getEntryPoint(), name, ext));
        try (PrintWriter p = new PrintWriter(out, "UTF-8")) {
            p.println("// " + f.getName(true) + " @ " + f.getEntryPoint() + "  insns=" + size(f));
            p.println("// xref: " + currentProgram.getReferenceManager()
                    .getReferenceCountTo(f.getEntryPoint()));
            p.println(r.getDecompiledFunction() != null ? r.getDecompiledFunction().getC()
                      : "// DECOMPILE FAILED: " + r.getErrorMessage());
        } catch (Exception e) {
            println("emit fallo " + f.getName(true) + ": " + e);
        }
        if (monitor != null && (idx % 10 == 0)) println("  ... " + idx + " " + f.getName(true));
    }
}