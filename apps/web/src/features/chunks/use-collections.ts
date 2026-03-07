import { useLocalStorage } from "@/hooks/use-local-storage";

export interface Collection {
    id: string;
    name: string;
    chunkIds: string[];
}

export function useCollections() {
    const [collections, setCollections] = useLocalStorage<Collection[]>("fubbik:collections", []);

    function createCollection(name: string) {
        const id = crypto.randomUUID();
        setCollections(prev => [...prev, { id, name, chunkIds: [] }]);
        return id;
    }

    function deleteCollection(id: string) {
        setCollections(prev => prev.filter(c => c.id !== id));
    }

    function addToCollection(collectionId: string, chunkIds: string[]) {
        setCollections(prev => prev.map(c => (c.id === collectionId ? { ...c, chunkIds: [...new Set([...c.chunkIds, ...chunkIds])] } : c)));
    }

    function removeFromCollection(collectionId: string, chunkId: string) {
        setCollections(prev => prev.map(c => (c.id === collectionId ? { ...c, chunkIds: c.chunkIds.filter(id => id !== chunkId) } : c)));
    }

    return { collections, createCollection, deleteCollection, addToCollection, removeFromCollection };
}
