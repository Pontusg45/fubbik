package sample;

/** Looks up stored values. */
public class Lookup {
    /** Find by numeric identifier.
     * @param id the identifier
     * @return the stored value
     */
    public String find(int id) { return ""; }

    /** Find by name.
     * @param name the lookup name
     * @return the stored value
     * @throws IllegalArgumentException if the name is invalid
     */
    public String find(String name) { return ""; }

    /** Internal implementation detail. */
    private void internal() {}
}
