@tool
class_name DevToolsChannel extends LoggieMsgChannel

class LoggieMsgTuple:
	var msg: LoggieMsg
	var type: LoggieEnums.MsgType

var _log_buffer: Array[LoggieMsgTuple]

func _init() -> void:
	self.ID = "devtools"
	self.preprocess_flags = LoggieEnums.PreprocessStep.APPEND_DOMAIN_NAME | LoggieEnums.PreprocessStep.APPEND_TIMESTAMPS

func clear() -> void:
	self._log_buffer.clear()

func get_all_logs() -> Array[LoggieMsgTuple]:
	return self._log_buffer

func get_logs_with_domain(domain: String) -> Array[LoggieMsgTuple]:
	var filtered_logs: Array[LoggieMsgTuple] = []
	for tuple in self._log_buffer:
		if tuple.msg.domain_name == domain:
			filtered_logs.append(tuple)
	return filtered_logs

func send(msg : LoggieMsg, type : LoggieEnums.MsgType) -> void:
	var _text = msg.last_preprocess_result
	var tuple = LoggieMsgTuple.new()
	tuple.msg = msg
	tuple.type = type
	self._log_buffer.append(tuple)
