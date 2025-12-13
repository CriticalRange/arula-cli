package com.arula.terminal;

/**
 * Represents a chat message
 */
public class Message {
    private final long id;
    private String text;
    private final Type type;
    private final long timestamp;
    private String toolId;

    public enum Type {
        USER,
        ASSISTANT,
        TOOL,
        ERROR
    }

    public Message(String text, Type type) {
        this(System.currentTimeMillis(), text, type, System.currentTimeMillis(), null);
    }

    public Message(long id, String text, Type type, long timestamp, String toolId) {
        this.id = id;
        this.text = text;
        this.type = type;
        this.timestamp = timestamp;
        this.toolId = toolId;
    }

    public long getId() {
        return id;
    }

    public String getText() {
        return text;
    }

    public Type getType() {
        return type;
    }

    public long getTimestamp() {
        return timestamp;
    }

    public String getToolId() {
        return toolId;
    }

    public void setToolId(String toolId) {
        this.toolId = toolId;
    }

    public void appendText(String append) {
        this.text += append;
    }

    @Override
    public boolean equals(Object obj) {
        if (this == obj) return true;
        if (obj == null || getClass() != obj.getClass()) return false;

        Message message = (Message) obj;

        if (id != message.id) return false;
        if (!text.equals(message.text)) return false;
        if (type != message.type) return false;
        if (timestamp != message.timestamp) return false;
        return toolId != null ? toolId.equals(message.toolId) : message.toolId == null;
    }

    @Override
    public int hashCode() {
        int result = (int) (id ^ (id >>> 32));
        result = 31 * result + text.hashCode();
        result = 31 * result + type.hashCode();
        result = 31 * result + (int) (timestamp ^ (timestamp >>> 32));
        result = 31 * result + (toolId != null ? toolId.hashCode() : 0);
        return result;
    }
}